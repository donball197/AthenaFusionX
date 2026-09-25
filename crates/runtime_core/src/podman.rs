use std::io::{Error, ErrorKind, Read, Result};
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

// Dedicated no-login, no-home user for executing Strategist-proposed
// commands. PRoot cannot provide namespace isolation (no real container
// runtime is possible in this environment -- confirmed: no cgroup
// controllers, no podman binary), so this is the only privilege boundary
// achievable here: bound the blast radius of anything that slips past
// ArbiterAgent::validate_action to what uid/gid 10001 can touch, instead
// of full root access to the filesystem. Create with:
//   useradd -u 10001 -g 10001 -M -N -s /usr/sbin/nologin nexus_exec
const EXEC_UID: u32 = 10001;
const EXEC_GID: u32 = 10001;

pub struct PodmanShield;

// No command run through this shield may block indefinitely. Anything that
// hangs -- a stuck compile, an interactive prompt waiting on stdin, a network
// call with no timeout of its own -- previously had no circuit breaker at
// all, which surfaced as an indistinguishable-from-hung cortex session.
// Compiles on this workspace have been observed taking 3-4+ minutes clean,
// so this needs real headroom rather than a short default.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(420);

impl PodmanShield {
    pub fn execute(command: &str) -> Result<Output> {
        Self::execute_with_timeout(command, DEFAULT_TIMEOUT)
    }

    pub fn execute_with_timeout(command: &str, timeout: Duration) -> Result<Output> {
        if let Err(e) =
            crate::arbiter_agent::ArbiterAgent::validate_action("execute_command", command)
        {
            return Err(Error::new(ErrorKind::PermissionDenied, e));
        }
        // Use bash, not POSIX sh -- the Strategist routinely proposes bash-only
        // syntax (${PIPESTATUS[0]}, the `time` keyword, etc.) which fails under
        // sh with cryptic errors like "Bad substitution", wasting turns before
        // any working command is found.
        let mut child_cmd = Command::new("bash");
        child_cmd.arg("-c").arg(command);
        for (key, value) in std::env::vars() {
            if key != "CARGO_BUILD_TARGET" {
                child_cmd.env(key, value);
            }
        }
        child_cmd.env_remove("CARGO_BUILD_TARGET");
        child_cmd.stdout(Stdio::piped());
        child_cmd.stderr(Stdio::piped());

        // SAFETY: pre_exec runs in the forked child between fork() and
        // exec(), before any other threads exist in that process image --
        // only async-signal-safe calls are permitted here. setgid()/setuid()
        // are both async-signal-safe. No allocation, no locking, nothing
        // else happens in this closure.
        unsafe {
            child_cmd.pre_exec(|| {
                if libc::setgid(EXEC_GID) != 0 {
                    return Err(Error::last_os_error());
                }
                if libc::setuid(EXEC_UID) != 0 {
                    return Err(Error::last_os_error());
                }
                Ok(())
            });
        }

        let mut child = child_cmd.spawn()?;

        // Drain stdout/stderr on background threads concurrently with the
        // process running. Without this, a command producing more output
        // than the OS pipe buffer (~64KB on Linux) would block the child on
        // write() while nothing reads -- a deadlock indistinguishable from
        // the hang this timeout exists to prevent.
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();

        let (stdout_tx, stdout_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut h) = stdout_pipe {
                let _ = h.read_to_end(&mut buf);
            }
            let _ = stdout_tx.send(buf);
        });

        let (stderr_tx, stderr_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut h) = stderr_pipe {
                let _ = h.read_to_end(&mut buf);
            }
            let _ = stderr_tx.send(buf);
        });

        let start = Instant::now();
        let status = loop {
            match child.try_wait()? {
                Some(status) => break status,
                None => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(Error::new(
                            ErrorKind::TimedOut,
                            format!(
                                "Command exceeded {:?} timeout and was killed: {}",
                                timeout, command
                            ),
                        ));
                    }
                    thread::sleep(Duration::from_millis(200));
                }
            }
        };

        let stdout = stdout_rx.recv().unwrap_or_default();
        let stderr = stderr_rx.recv().unwrap_or_default();
        let out = Output { status, stdout, stderr };

        // REGRESSION FIX (reapplied): explicitly check command success and
        // expose output. Without this, a failing command (e.g. a broken
        // `cargo check`) is indistinguishable from a successful one -- both
        // get tagged [CMD_OK] by the executor, which the Cortical Loop's
        // is_terminal_cmd heuristic then treats as proof of success.
        //
        // Include BOTH stdout and stderr in the error, not stderr alone.
        // Callers that redirect 2>&1 in their own command string (e.g.
        // RunTests) merge all output into stdout at the shell level -- an
        // error message built from stderr only then reports a bare exit
        // code with no diagnostic content, hiding the actual compiler
        // error from the agent entirely and leaving it unable to diagnose
        // real failures (confirmed live: a genuine compile error was
        // completely invisible across 13 turns because of this gap).
        if !out.status.success() {
            let stdout_str = String::from_utf8(out.stdout.clone())
                .unwrap_or_else(|_| "[INVALID UTF-8 IN STDOUT]".to_string());
            let stderr_str = String::from_utf8(out.stderr.clone())
                .unwrap_or_else(|_| "[INVALID UTF-8 IN STDERR]".to_string());
            return Err(Error::other(format!(
                "Command failed with status: {}\nSTDOUT:\n{}\nSTDERR:\n{}",
                out.status, stdout_str, stderr_str
            )));
        }
        Ok(out)
    }
}
