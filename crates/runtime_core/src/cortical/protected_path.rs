//! PROTECTED PATH GATE — vendored trees are read-only unless the
//! contract names the exact path as its artifact.
//!
//! This runs in `handle_critic` before the LLM critic and before any
//! `[CRITIC FACT OVERRIDE]` branch. It is the structural defence against
//! the failure class that produced two forbidden vendor mutations during
//! the OpenFang missions: a mutation action targeting `vendor/` (or
//! `third_party/`) which the LLM critic vetoes, then the cortex overrides
//! via FACT_OVERRIDE and the mutation lands anyway.

use crate::agent_protocol::AgentAction;

const PROTECTED_DIRS: &[&str] = &["vendor", "third_party"];
const MUTATING_KINDS: &[&str] =
    &["write_file", "patch_file", "replace_lines", "atomic_patch", "diff_patch", "patch_function"];
const PATH_KEYS: &[&str] = &["path", "file_path", "filepath", "file", "target_path", "target"];

fn normalize_path(p: &str) -> String {
    let mut s = p.trim().replace('\\', "/");
    while let Some(r) = s.strip_prefix("./") {
        s = r.to_string();
    }
    s
}

/// True when `path` is inside a protected tree, spelled relative or absolute.
pub fn is_protected_path(path: &str) -> bool {
    let n = normalize_path(path);
    PROTECTED_DIRS
        .iter()
        .any(|d| n == *d || n.starts_with(&format!("{d}/")) || n.contains(&format!("/{d}/")))
}

fn violation_in_json(
    v: &serde_json::Value,
    allowed: &[String],
    shell_target: fn(&str) -> Option<String>,
) -> Option<String> {
    let (kind, body): (String, &serde_json::Value) = match v.get("type").and_then(|t| t.as_str()) {
        Some(t) => (t.to_string(), v),
        None => {
            let obj = v.as_object()?;
            if obj.len() != 1 {
                return None;
            }
            let (k, inner) = obj.iter().next()?;
            (k.clone(), inner)
        }
    };
    let path: Option<String> = if MUTATING_KINDS.contains(&kind.as_str()) {
        PATH_KEYS.iter().find_map(|k| body.get(*k).and_then(|x| x.as_str())).map(str::to_string)
    } else if kind == "execute_command" {
        body.get("command").and_then(|c| c.as_str()).and_then(shell_target)
    } else {
        None
    };
    if kind == "execute_command" {
        if let Some(target) = body
            .get("command")
            .and_then(|c| c.as_str())
            .and_then(removal_violation_target)
        {
            if !allowed.iter().any(|a| normalize_path(a) == normalize_path(&target)) {
                return Some(format!(
                    "PROTECTED PATH GATE: `execute_command` that deletes or moves `{}` is refused. Whole protected directories (source, CI, config, tests, vendored trees) must never be removed or relocated; delete individual files inside them only when the task requires it.",
                    target
                ));
            }
        }
    }
    let path = path?;
    let n = normalize_path(&path);
    if !is_protected_path(&n) {
        return None;
    }
    if allowed.iter().any(|a| normalize_path(a) == n) {
        return None;
    }
    Some(format!(
        "PROTECTED PATH GATE: `{}` on `{}` is refused. Files under vendor/ or third_party/ are read-only unless the contract names the exact path as its artifact. Inspect them with read_file, read_file_chunk or search_code; do not modify them.",
        kind, path
    ))
}

/// `Some(reason)` when `action` would modify a protected tree and the contract does not
/// name that exact path as an artifact. Reads and searches are never refused.
pub fn protected_violation(action: &AgentAction, allowed: &[String]) -> Option<String> {
    let v = serde_json::to_value(action).ok()?;
    violation_in_json(&v, allowed, crate::executor::detect_shell_write_target)
}

// ---- Whole-tree removal gate ------------------------------------------------
// Deleting or relocating a protected ROOT (lib, config, tests, .github, ...) is
// refused for shell commands. Deleting individual files inside them is not.
// Directory list is shared with the report scan (single source of truth).

fn rel_to_cwd(n: &str) -> String {
    if n.starts_with('/') {
        if let Ok(cwd) = std::env::current_dir() {
            let c = cwd.to_string_lossy().replace('\\', "/");
            if n == c {
                return ".".to_string();
            }
            if let Some(r) = n.strip_prefix(&format!("{c}/")) {
                return r.to_string();
            }
        }
    }
    n.to_string()
}

fn is_removal_root(path: &str) -> bool {
    let unquoted = path.trim_matches(|c| c == '"' || c == '\'');
    let mut n = rel_to_cwd(&normalize_path(unquoted));
    for suffix in ["/*", "/."] {
        if let Some(r) = n.strip_suffix(suffix) {
            n = r.to_string();
        }
    }
    while n.len() > 1 && n.ends_with('/') {
        n.pop();
    }
    let lower = n.to_ascii_lowercase();
    if matches!(lower.as_str(), "." | ".." | "*" | "/" | "~" | "$home" | "${home}") {
        return true;
    }
    super::forbidden_removal::FORBIDDEN_REMOVAL_DIRS.iter().any(|d| n == *d)
}

fn non_flags(a: &[&str]) -> Vec<String> {
    a.iter().filter(|t| !t.starts_with('-')).map(|t| t.to_string()).collect()
}

fn removal_targets(segment: &str) -> Vec<String> {
    let mut toks: Vec<&str> = segment.split_whitespace().collect();
    while let Some(t) = toks.first() {
        if matches!(*t, "sudo" | "env" | "time" | "nohup" | "command" | "exec")
            || (t.contains('=') && !t.starts_with('-'))
        {
            toks.remove(0);
        } else {
            break;
        }
    }
    if toks.is_empty() {
        return vec![];
    }
    let cmd = toks[0].rsplit('/').next().unwrap_or("");
    let args = &toks[1..];
    match cmd {
        "rm" | "rmdir" | "unlink" | "shred" => non_flags(args),
        "git" => match args.first().copied() {
            Some("rm") => non_flags(&args[1..]),
            Some("clean") => {
                let t = non_flags(&args[1..]);
                if t.is_empty() { vec![".".to_string()] } else { t }
            }
            _ => vec![],
        },
        "mv" => {
            let t = non_flags(args);
            if t.len() > 1 { t[..t.len() - 1].to_vec() } else { vec![] }
        }
        "find" => {
            let destructive = args.iter().any(|t| *t == "-delete")
                || args
                    .windows(2)
                    .any(|w| w[0] == "-exec" && (w[1] == "rm" || w[1].ends_with("/rm")));
            if !destructive {
                return vec![];
            }
            let paths: Vec<String> = args
                .iter()
                .take_while(|t| !t.starts_with('-') && !t.starts_with('(') && !t.starts_with('!'))
                .map(|t| t.to_string())
                .collect();
            if paths.is_empty() { vec![".".to_string()] } else { paths }
        }
        "sh" | "bash" | "zsh" => {
            if let Some(i) = args.iter().position(|t| *t == "-c") {
                let inner = args[i + 1..].join(" ");
                let inner = inner.trim_matches(|c| c == '"' || c == '\'').to_string();
                return removal_violation_target(&inner).into_iter().collect();
            }
            vec![]
        }
        _ => vec![],
    }
}

/// First protected root that `command` would delete or move away, if any.
pub fn removal_violation_target(command: &str) -> Option<String> {
    let flat = command
        .replace("&&", "\n")
        .replace("||", "\n")
        .replace(|c: char| c == '|' || c == ';', "\n");
    flat.lines().flat_map(removal_targets).find(|t| is_removal_root(t))
}

#[cfg(test)]
mod removal_tests {
    use super::*;

    #[test]
    fn refuses_removal_of_protected_roots() {
        for cmd in [
            "rm -rf lib/", "rm -rf ./vendor", "cd x && rm -rf crates", "git rm -r config",
            "git clean -fdx", "mv lib /tmp/lib", "/bin/rm -rf lib", "sudo rm -rf data/",
            "rm -rf *", "find . -delete", "bash -c \"rm -rf tests\"", "FOO=1 rm -rf docs",
            "rm -rf lib/*",
        ] {
            assert!(removal_violation_target(cmd).is_some(), "should refuse: {cmd}");
        }
    }

    #[test]
    fn allows_ordinary_commands() {
        for cmd in [
            "rm -rf target", "rm tests/tmp.txt", "mv /tmp/a lib", "ls lib",
            "echo rm -rf lib", "find tests/tmp -delete", "rm -rf crates/runtime_core/tmp",
            "git status", "cargo test",
        ] {
            assert!(removal_violation_target(cmd).is_none(), "should allow: {cmd}");
        }
    }
}

#[cfg(test)]
mod protected_path_tests {
    use super::*;
    use serde_json::json;

    fn no_shell(_: &str) -> Option<String> {
        None
    }
    fn redirect_shell(c: &str) -> Option<String> {
        c.split_once("> ").map(|(_, p)| p.trim().to_string())
    }
    fn check(v: serde_json::Value, allowed: &[&str]) -> Option<String> {
        let allowed: Vec<String> = allowed.iter().map(|s| s.to_string()).collect();
        violation_in_json(&v, &allowed, no_shell)
    }

    #[test]
    fn blocks_the_openfang_patch_shape() {
        let v = json!({"type":"patch_function","path":"/root/AthenaFusionX/vendor/openfang/crates/openfang-runtime/src/host_functions.rs","function_name":"dispatch","new_body":"fn dispatch() {}"});
        assert!(check(v, &[]).is_some());
    }

    #[test]
    fn blocks_relative_and_dot_slash_spellings() {
        assert!(
            check(json!({"type":"replace_lines","path":"./vendor/openfang/x.rs"}), &[]).is_some()
        );
        assert!(check(
            json!({"type":"write_file","path":"vendor/openfang/Cargo.toml","content":"x"}),
            &[]
        )
        .is_some());
        assert!(check(json!({"type":"diff_patch","path":"third_party/lib/a.c"}), &[]).is_some());
    }

    #[test]
    fn reads_and_searches_are_never_refused() {
        assert!(
            check(json!({"type":"read_file","path":"vendor/openfang/Cargo.toml"}), &[]).is_none()
        );
        assert!(check(
            json!({"type":"read_file_chunk","path":"vendor/openfang/a.rs","start_line":1}),
            &[]
        )
        .is_none());
        assert!(check(json!({"type":"search_code","path":"vendor/openfang","query":"wasm"}), &[])
            .is_none());
    }

    #[test]
    fn ordinary_edits_and_lookalike_directories_pass() {
        assert!(check(json!({"type":"patch_function","path":"crates/runtime_core/src/x.rs","function_name":"f","new_body":"x"}), &[]).is_none());
        assert!(check(
            json!({"type":"write_file","path":"crates/vendor_tools/src/lib.rs","content":"x"}),
            &[]
        )
        .is_none());
        assert!(check(
            json!({"type":"write_file","path":"ROADMAP_OPENFANG_COMPARISON.md","content":"x"}),
            &[]
        )
        .is_none());
    }

    #[test]
    fn named_contract_artifact_is_allowed() {
        let v = || json!({"type":"write_file","path":"vendor/openfang/NOTES.md","content":"x"});
        assert!(check(v(), &["vendor/openfang/NOTES.md"]).is_none());
        assert!(check(v(), &["./vendor/openfang/NOTES.md"]).is_none());
        assert!(check(v(), &["vendor/openfang/OTHER.md"]).is_some());
    }

    #[test]
    fn externally_tagged_shape_is_understood() {
        assert!(check(json!({"write_file":{"path":"vendor/a.txt","content":"x"}}), &[]).is_some());
    }

    #[test]
    fn shell_redirection_into_vendor_is_refused() {
        let cmd = |c: &str| json!({"type":"execute_command","command":c});
        assert!(violation_in_json(&cmd("echo hi > vendor/openfang/a.txt"), &[], redirect_shell)
            .is_some());
        assert!(violation_in_json(&cmd("ls vendor/openfang/crates"), &[], redirect_shell).is_none());
        assert!(violation_in_json(&cmd("echo hi > /tmp/out.txt"), &[], redirect_shell).is_none());
    }
}

#[cfg(test)]
mod removal_wiring_tests {
    use super::*;

    fn run(cmd: &str, allowed: &[String]) -> Option<String> {
        let v = serde_json::json!({ "type": "execute_command", "command": cmd });
        violation_in_json(&v, allowed, crate::executor::detect_shell_write_target)
    }

    #[test]
    fn execute_command_removal_of_a_root_is_refused_through_the_gate() {
        let msg = run("rm -rf lib/", &[]).expect("must be refused");
        assert!(msg.contains("PROTECTED PATH GATE"), "{msg}");
        assert!(run("cd x && git rm -r config", &[]).is_some());
    }

    #[test]
    fn execute_command_ordinary_deletes_pass_the_gate() {
        assert!(run("rm -rf target", &[]).is_none());
        assert!(run("rm tests/tmp.txt", &[]).is_none());
        assert!(run("cargo test", &[]).is_none());
    }

    #[test]
    fn contract_named_path_is_allowed() {
        assert!(run("rm -rf lib", &["lib".to_string()]).is_none());
    }
}
