//! READ-ONLY mission enforcement helpers (pure, no `self`).
//!
//! An allowlist screen for shell commands. It narrows what a read-only
//! mission can run; it is NOT a sandbox (see the PodmanShield note).

use crate::agent_protocol::AgentAction;
use crate::cortical::capability::is_mutation;

const SAFE_COMMANDS: &[&str] = &[
    "grep", "egrep", "fgrep", "cat", "ls", "wc", "head", "tail", "find", "sort", "stat", "pwd",
    "echo", "test", "tr", "cut", "nl", "diff", "basename", "dirname", "realpath", "git",
];
const SAFE_GIT: &[&str] = &[
    "log",
    "diff",
    "show",
    "status",
    "ls-files",
    "grep",
    "blame",
    "rev-parse",
    "rev-list",
    "describe",
];
const BAD_FIND: &[&str] =
    &["-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprint", "-fprint0", "-fprintf", "-fls"];

fn norm(tok: &str) -> String {
    tok.chars().filter(|c| !matches!(c, '"' | '\'' | '\\')).collect()
}

/// True only for pipelines of allowlisted read-only commands.
pub(crate) fn is_read_only_safe_command(cmd: &str) -> bool {
    let chars: Vec<char> = cmd.chars().collect();
    let mut segments: Vec<String> = Vec::new();
    let mut cur = String::new();
    let (mut single, mut double) = (false, false);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if single {
            if c == '\'' {
                single = false;
            }
            cur.push(c);
            i += 1;
            continue;
        }
        if c == '\\' {
            cur.push(c);
            if let Some(n) = chars.get(i + 1) {
                cur.push(*n);
            }
            i += 2;
            continue;
        }
        if double {
            if c == '"' {
                double = false;
            } else if c == '`' || (c == '$' && chars.get(i + 1) == Some(&'(')) {
                return false;
            }
            cur.push(c);
            i += 1;
            continue;
        }
        let rest: String = chars[i..].iter().take(11).collect();
        if rest.starts_with("2>&1") {
            i += 4;
            continue;
        }
        if rest.starts_with("2>/dev/null") {
            i += 11;
            continue;
        }
        match c {
            '\'' => {
                single = true;
                cur.push(c);
            }
            '"' => {
                double = true;
                cur.push(c);
            }
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    return false;
                }
                segments.push(std::mem::take(&mut cur));
            }
            '>' | '<' | ';' | '&' | '`' | '\n' | '\r' => return false,
            '$' if chars.get(i + 1) == Some(&'(') => return false,
            _ => cur.push(c),
        }
        i += 1;
    }
    if single || double {
        return false;
    }
    segments.push(cur);

    for seg in &segments {
        let toks: Vec<String> = seg.split_whitespace().map(norm).collect();
        let Some(first) = toks.first() else {
            return false;
        };
        if !SAFE_COMMANDS.contains(&first.as_str()) {
            return false;
        }
        let args = &toks[1..];
        match first.as_str() {
            "find" if args.iter().any(|t| BAD_FIND.contains(&t.as_str())) => return false,
            "sort"
                if args.iter().any(|t| {
                    t.starts_with("--output")
                        || (t.starts_with('-') && !t.starts_with("--") && t.contains('o'))
                }) =>
            {
                return false
            }
            "git" => {
                if !args.first().is_some_and(|s| SAFE_GIT.contains(&s.as_str())) {
                    return false;
                }
                if args.iter().any(|t| {
                    t.starts_with("--output")
                        || t.starts_with("--ext-diff")
                        || t.starts_with("--open-files-in-pager")
                        || t.starts_with("-O")
                }) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// True when `action` must be refused under a READ-ONLY contract.
pub(crate) fn violates_read_only(action: &AgentAction) -> bool {
    match action {
        AgentAction::ExecuteCommand { command, .. } => !is_read_only_safe_command(command),
        // `run_tests` is an execution-side-effect operation, not a
        // substrate-read operation. READ-ONLY missions must not invoke it.
        AgentAction::RunTests { .. } => true,
        other => is_mutation(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_command_screening() {
        for ok in [
            r#"grep -nE '^\s*(pub(\(crate\))? )?(async )?fn ' crates/runtime_core/src/cortical_loop.rs"#,
            "grep -rn foo . | sort | head -20",
            "ls -la 2>&1",
            "cat a.rs",
            "git log --oneline",
            "find . -name '*.rs'",
            "grep -c fn file.rs 2>/dev/null",
            "echo 'a > b'",
        ] {
            assert!(is_read_only_safe_command(ok), "should allow {:?}", ok);
        }
        for bad in [
            "echo x > f",
            "echo x>f",
            "cat a; rm b",
            "ls && rm f",
            "sed -i s/a/b/ f",
            "find . -exec rm {} +",
            r#"find . "-exec" rm {} +"#,
            "find . -delete",
            "echo $(id)",
            "echo `id`",
            r#"echo "$(id)""#,
            "grep x f | tee out",
            "sort -o f a",
            "sort -ro f a",
            "git push",
            "git -c core.pager=x log",
            "python3 -c 1",
            "cat <<EOF",
            "ls |",
            "",
            "echo 'unterminated",
            "grep x f || rm f",
            "cat a\nrm b",
        ] {
            assert!(!is_read_only_safe_command(bad), "should reject {:?}", bad);
        }
    }

    #[test]
    fn run_tests_is_rejected_under_read_only() {
        let action = AgentAction::RunTests {
            test_target: "runtime_core".to_string(),
            _thought_process: None,
            reasoning: "verify implementation".to_string(),
        };

        assert!(violates_read_only(&action), "RunTests must be rejected under READ_ONLY");
    }
}
