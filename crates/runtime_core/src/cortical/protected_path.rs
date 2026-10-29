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
