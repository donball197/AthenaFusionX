//! Isolated remediation sandbox and optional adversarial PoC verification.

use crate::client::ApiClient;
use crate::polyglot::{SecurityFinding, SupportedLanguage};
use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct PocOutcome {
    pub executed: bool,
    pub exploited_before: Option<bool>,
    pub blocked_after: Option<bool>,
    pub detail: String,
}

pub struct HealSandbox {
    pub authoritative_root: PathBuf,
    pub sandbox_root: PathBuf,
    single_file_mode: bool,
}

impl HealSandbox {
    pub fn create(target: &Path) -> Result<Self> {
        let canonical = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let pid = std::process::id();
        let sandbox_root =
            std::env::temp_dir().join(format!("nexus_heal_sandbox_{}_{}", pid, ts));
        fs::create_dir_all(&sandbox_root)?;

        let single_file_mode = canonical.is_file();
        if single_file_mode {
            let name = canonical
                .file_name()
                .ok_or_else(|| anyhow!("Target file has no file name"))?;
            fs::copy(&canonical, sandbox_root.join(name))?;
        } else {
            copy_dir_filtered(&canonical, &sandbox_root)?;
        }

        Ok(Self {
            authoritative_root: canonical,
            sandbox_root,
            single_file_mode,
        })
    }

    pub fn sandbox_path(&self, authoritative_file: &Path) -> Result<PathBuf> {
        let abs = authoritative_file
            .canonicalize()
            .unwrap_or_else(|_| authoritative_file.to_path_buf());
        if self.single_file_mode {
            let name = abs
                .file_name()
                .ok_or_else(|| anyhow!("File has no name: {}", abs.display()))?;
            return Ok(self.sandbox_root.join(name));
        }
        let rel = abs.strip_prefix(&self.authoritative_root).map_err(|_| {
            anyhow!(
                "Path {} is outside sandbox root {}",
                abs.display(),
                self.authoritative_root.display()
            )
        })?;
        Ok(self.sandbox_root.join(rel))
    }

    pub fn promote(&self, modified_authoritative_files: &[PathBuf]) -> Result<usize> {
        let mut promoted = 0;
        for auth_path in modified_authoritative_files {
            let sb_path = self.sandbox_path(auth_path)?;
            if sb_path.exists() {
                fs::copy(&sb_path, auth_path)?;
                promoted += 1;
            }
        }
        Ok(promoted)
    }
}

impl Drop for HealSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.sandbox_root);
    }
}

fn copy_dir_filtered(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if matches!(
            name_str.as_ref(),
            ".git"
                | "target"
                | "node_modules"
                | ".venv"
                | "venv"
                | "__pycache__"
                | "dist"
                | "build"
                | ".nexus"
        ) {
            continue;
        }
        let dst_path = dst.join(&name);
        if path.is_dir() {
            copy_dir_filtered(&path, &dst_path)?;
        } else if path.is_file() {
            fs::copy(&path, &dst_path)?;
        }
    }
    Ok(())
}

/// Generate and run a self-contained PoC check comparing pre-patch vs post-patch snippets.
///
/// The PoC runner uses a hard 8-second timeout and runs in an isolated temp directory.
pub fn verify_poc_against_patch(
    finding: &SecurityFinding,
    pre_snippet: &str,
    post_snippet: &str,
) -> PocOutcome {
    let Some((runner, ext)) = poc_runner_for_lang(finding.language) else {
        return PocOutcome {
            executed: false,
            exploited_before: None,
            blocked_after: None,
            detail: format!("PoC verification not yet supported for {:?}", finding.language),
        };
    };

    let pre_owned = dedent_snippet(pre_snippet);
    let post_owned = dedent_snippet(post_snippet);
    let pre_snippet = pre_owned.as_str();
    let post_snippet = post_owned.as_str();
    let lang_str = format!("{:?}", finding.language);
    let pre_script = match ApiClient::generate_poc_script(
        &finding.pattern_name,
        pre_snippet,
        &lang_str,
        "vulnerable",
    ) {
        Ok(s) => s,
        Err(e) => {
            return PocOutcome {
                executed: false,
                exploited_before: None,
                blocked_after: None,
                detail: format!("Failed to generate pre-patch PoC: {}", e),
            };
        }
    };

    let post_script = match ApiClient::generate_poc_script(
        &finding.pattern_name,
        post_snippet,
        &lang_str,
        "patched",
    ) {
        Ok(s) => s,
        Err(e) => {
            return PocOutcome {
                executed: false,
                exploited_before: None,
                blocked_after: None,
                detail: format!("Failed to generate post-patch PoC: {}", e),
            };
        }
    };

    let pre_ok = run_timed_script(runner, ext, &pre_script, Duration::from_secs(8));
    let post_ok = run_timed_script(runner, ext, &post_script, Duration::from_secs(8));

    match (pre_ok, post_ok) {
        (Ok((before_passed, pre_err)), Ok((after_passed, post_err))) => {
            let mut detail = format!(
                "Pre-patch PoC ok={}, Post-patch verification ok={}",
                before_passed, after_passed
            );
            if !after_passed && !post_err.trim().is_empty() {
                detail.push_str(&format!(" ({})", post_err.trim()));
            } else if !before_passed && !pre_err.trim().is_empty() {
                detail.push_str(&format!(" ({})", pre_err.trim()));
            }
            PocOutcome {
                executed: true,
                exploited_before: Some(before_passed),
                blocked_after: Some(after_passed),
                detail,
            }
        }
        (Err(e), _) | (_, Err(e)) => PocOutcome {
            executed: false,
            exploited_before: None,
            blocked_after: None,
            detail: format!("PoC runner error: {}", e),
        },
    }
}

/// Strip common leading indentation so a snippet cut from inside a block can be exec'd standalone.
fn dedent_snippet(s: &str) -> String {
    let indent = s
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    s.lines()
        .map(|l| if l.len() >= indent { &l[indent..] } else { l.trim_start() })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Static checks on a patched Python file vs. the original.
/// Returns reasons to reject: new pyflakes undefined names, or new SQL identifiers
/// (after FROM/INTO/UPDATE/JOIN) that the original file did not contain.
pub fn patch_static_issues(old_src: &str, new_src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let base: std::collections::HashSet<String> = pyflakes_undefined(old_src).into_iter().collect();
    for m in pyflakes_undefined(new_src) {
        if !base.contains(&m) {
            out.push(format!("pyflakes: {}", m));
        }
    }
    let old_t = sql_idents(old_src);
    for t in sql_idents(new_src) {
        if !old_t.contains(&t) {
            out.push(format!("new SQL identifier '{}' not present in original", t));
        }
    }
    out
}

fn pyflakes_undefined(src: &str) -> Vec<String> {
    use std::io::Write;
    let Ok(mut child) = Command::new("python3")
        .args(["-m", "pyflakes"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return vec![];
    };
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(src.as_bytes());
    }
    let Ok(o) = child.wait_with_output() else { return vec![] };
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| l.contains("undefined name"))
        .map(|l| l.splitn(4, ':').nth(3).unwrap_or(l).trim().to_string())
        .collect()
}

fn sql_idents(src: &str) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("from ") || t.starts_with("import ") {
            continue;
        }
        let toks: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || "\"'`(),".contains(c))
            .filter(|x| !x.is_empty())
            .collect();
        for w in toks.windows(2) {
            let kw = w[0].to_ascii_uppercase();
            if kw == "FROM" || kw == "INTO" || kw == "UPDATE" || kw == "JOIN" {
                let id = w[1].trim_matches(|c: char| !(c.is_alphanumeric() || c == '_'));
                let first = w[1].chars().next().unwrap_or(' ');
                if !id.is_empty() && first != '{' && first != ':' && first != '%' && first != '$' && first != '?' {
                    set.insert(id.to_ascii_lowercase());
                }
            }
        }
    }
    set
}

fn poc_runner_for_lang(lang: SupportedLanguage) -> Option<(&'static str, &'static str)> {
    match lang {
        SupportedLanguage::Python => Some(("python3", "py")),
        SupportedLanguage::JavaScript => Some(("node", "js")),
        SupportedLanguage::PHP => Some(("php", "php")),
        _ => None,
    }
}

fn run_timed_script(
    runner: &str,
    ext: &str,
    script: &str,
    timeout: Duration,
) -> Result<(bool, String)> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let script_path = std::env::temp_dir().join(format!("nexus_poc_{}.{}", ts, ext));
    fs::write(&script_path, script)?;
    if runner == "python3" {
        let chk = Command::new("python3").args(["-m", "py_compile"]).arg(&script_path)
            .stdin(Stdio::null()).output()?;
        if !chk.status.success() {
            let msg = String::from_utf8_lossy(&chk.stderr).trim().to_string();
            if std::env::var("NEXUS_KEEP_POC").is_ok() {
                eprintln!("[poc kept] {}", script_path.display());
            } else {
                let _ = fs::remove_file(&script_path);
            }
            return Err(anyhow!("PoC script failed py_compile: {}", msg.lines().last().unwrap_or("")));
        }
    }

    let child = Command::new(runner).env("PYTHONWARNINGS", "ignore").arg(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut child = child;
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&script_path);
            return Err(anyhow!("PoC script timed out after {:?}", timeout));
        }
        thread::sleep(Duration::from_millis(25));
    }

    let output = child.wait_with_output()?;
    if !output.status.success() && std::env::var("NEXUS_KEEP_POC").is_ok() {
        eprintln!("[poc kept] {}\n[poc stderr]\n{}", script_path.display(), String::from_utf8_lossy(&output.stderr));
    } else {
        let _ = fs::remove_file(&script_path);
    }
    let err_tail = String::from_utf8_lossy(&output.stderr)
        .lines()
        .last()
        .unwrap_or("")
        .to_string();
    Ok((output.status.success(), err_tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_isolates_and_promotes_only_verified_files() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let auth = std::env::temp_dir().join(format!("nexus_sb_test_{}", ts));
        fs::create_dir_all(&auth).unwrap();
        let target = auth.join("app.py");
        fs::write(&target, "original = 1\n").unwrap();

        let sb = HealSandbox::create(&auth).unwrap();
        let sb_target = sb.sandbox_path(&target).unwrap();
        fs::write(&sb_target, "patched = 2\n").unwrap();

        // Authoritative must remain untouched until promote()
        assert_eq!(fs::read_to_string(&target).unwrap(), "original = 1\n");
        assert_eq!(sb.promote(std::slice::from_ref(&target)).unwrap(), 1);
        assert_eq!(fs::read_to_string(&target).unwrap(), "patched = 2\n");

        let _ = fs::remove_dir_all(&auth);
    }
}
