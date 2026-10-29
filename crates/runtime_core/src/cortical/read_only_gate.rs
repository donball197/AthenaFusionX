//! READ-ONLY mission enforcement helpers (pure, no `self`).
//!
//! An allowlist screen for shell commands. It narrows what a read-only
//! mission can run; it is NOT a sandbox (see the PodmanShield note).

use crate::agent_protocol::AgentAction;
use crate::cortical::capability::is_mutation;

const SAFE_COMMANDS: &[&str] = &[
    "grep", "egrep", "fgrep", "cat", "ls", "wc", "head", "tail", "find", "sort", "stat", "pwd",
    "echo", "test", "tr", "cut", "nl", "diff", "basename", "dirname", "realpath", "git",
    // Shell built-ins used to chain read-only recon in a single action:
    //   cd /repo && git status
    //   printf '===\n' && git status
    // `cd` changes only the shell's process cwd, not the repo. `printf`
    // writes to stdout (or to fd 0 as a formatting hint), not to the
    // filesystem. Neither can mutate a tracked file. `>` is still rejected
    // at the parser level, so `printf ... > file` remains refused.
    "cd", "printf",
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
    // `git branch` with no positional args lists branches (read-only).
    // `git branch <name>` would CREATE a branch, but the SAFE_GIT check
    // only gates the subcommand name; positional args that create/delete
    // branches must be rejected by the extended check below.
    "branch",
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
                // `||` — logical OR chain. Each side is an independent
                // read-only command; split and continue. A lone `|` is a
                // pipe: also split (downstream command analyzed on its own).
                segments.push(std::mem::take(&mut cur));
                if chars.get(i + 1) == Some(&'|') {
                    i += 2;
                    continue;
                }
            }
            '&' => {
                // `&&` — logical AND chain. Both sides are independently
                // evaluated; if either fails SAFE_COMMANDS, the whole
                // compound fails. A lone `&` (backgrounding) is refused.
                if chars.get(i + 1) == Some(&'&') {
                    segments.push(std::mem::take(&mut cur));
                    i += 2;
                    continue;
                }
                return false;
            }
            ';' => {
                // Sequential separator. Each segment is analyzed on its own
                // against SAFE_COMMANDS below, same as `&&`.
                if !cur.trim().is_empty() {
                    segments.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
            '>' | '<' | '`' | '\n' | '\r' => return false,
            '$' if chars.get(i + 1) == Some(&'(') => return false,
            _ => cur.push(c),
        }
        i += 1;
    }
    if single || double {
        return false;
    }
    // A trailing `;` leaves an empty tail segment; that is not a command.
    if !(cur.trim().is_empty() && cmd.trim_end().ends_with(';')) {
        segments.push(cur);
    }
    if segments.is_empty() {
        return false;
    }

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
                let Some(sub) = args.first() else {
                    return false;
                };
                if !SAFE_GIT.contains(&sub.as_str()) {
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
                // `git branch` without a positional is a listing (safe).
                // `git branch <name>` / `-d` / `-D` / `-m` / `-M` / `--delete`
                // / `--move` / `--copy` / `-c` / `--edit-description` /
                // `--set-upstream-to` are mutations and are refused.
                if sub.as_str() == "branch" {
                    let mutarg = args.iter().skip(1).any(|t| {
                        matches!(
                            t.as_str(),
                            "-d" | "-D"
                                | "-m"
                                | "-M"
                                | "-c"
                                | "-C"
                                | "-f"
                                | "--delete"
                                | "--move"
                                | "--copy"
                                | "--edit-description"
                                | "--set-upstream-to"
                                | "--unset-upstream"
                        ) || (!t.starts_with('-') && t != "branch")
                    });
                    if mutarg {
                        return false;
                    }
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

#[cfg(test)]
mod read_only_gate_constants_tests {
    use super::is_read_only_safe_command;

    // ---- POSITIVE: read-only audit needs these -----------------------
    #[test]
    fn git_readonly_subcommands_are_safe() {
        assert!(is_read_only_safe_command("git status --short"));
        assert!(is_read_only_safe_command("git branch"));
        assert!(is_read_only_safe_command("git branch --show-current"));
        assert!(is_read_only_safe_command("git rev-parse HEAD"));
        assert!(is_read_only_safe_command("git log -1 --oneline"));
        assert!(is_read_only_safe_command("git ls-files"));
        assert!(is_read_only_safe_command("git diff HEAD~1"));
    }

    #[test]
    fn chained_readonly_commands_are_safe() {
        assert!(is_read_only_safe_command(
            "git status --short && git branch --show-current && git rev-parse HEAD"
        ));
        assert!(is_read_only_safe_command("cd /root/AthenaFusionX && git status"));
        assert!(is_read_only_safe_command("printf '=== STATUS ===\n' && git status --short"));
        assert!(is_read_only_safe_command("find crates -name Cargo.toml | sort"));
    }

    // ---- NEGATIVE: the safety property the gate exists to uphold -----
    #[test]
    fn git_mutations_are_rejected() {
        assert!(!is_read_only_safe_command("git reset --hard HEAD"));
        assert!(!is_read_only_safe_command("git checkout main"));
        assert!(!is_read_only_safe_command("git push origin main"));
        assert!(!is_read_only_safe_command("git branch new-feature"));
        assert!(!is_read_only_safe_command("git branch -d old-feature"));
        assert!(!is_read_only_safe_command("git branch -D force-delete"));
        assert!(!is_read_only_safe_command("git branch -m rename"));
    }

    #[test]
    fn shell_mutations_are_rejected() {
        assert!(!is_read_only_safe_command("echo x > file"));
        assert!(!is_read_only_safe_command("printf 'x' > file"));
        assert!(!is_read_only_safe_command("rm -rf vendor"));
        assert!(!is_read_only_safe_command("cp a b"));
        assert!(!is_read_only_safe_command("mv a b"));
        assert!(!is_read_only_safe_command(r"find . -exec rm {} \;"));
        assert!(!is_read_only_safe_command("git status &"));
    }
}

#[cfg(test)]
mod semicolon_chain_tests {
    use super::is_read_only_safe_command as safe;

    #[test]
    fn a0_baseline_command_is_safe() {
        assert!(safe(
            r#"git status --short; echo '---BRANCH---'; git rev-parse --abbrev-ref HEAD; echo '---COMMIT---'; git rev-parse HEAD; echo '---INVENTORY---'; find crates -type f \( -name '*.rs' -o -name 'Cargo.toml' \) | sort; echo '---COUNT---'; find crates -type f -name '*.rs' | wc -l"#
        ));
    }

    #[test]
    fn unsafe_segment_after_semicolon_is_refused() {
        assert!(!safe("git status; rm -rf x"));
        assert!(!safe("ls; git branch newbranch"));
        assert!(!safe("echo hi; find . -delete"));
        assert!(!safe("cat a; sort -o out f"));
        assert!(!safe("echo a;git commit -m x"));
    }

    #[test]
    fn redirects_and_subshells_still_refused_with_semicolons() {
        assert!(!safe("ls; echo hi > f"));
        assert!(!safe("ls; echo $(id)"));
        assert!(!safe("ls; echo `id`"));
        assert!(!safe("ls & ls"));
    }

    #[test]
    fn trailing_and_quoted_semicolons() {
        assert!(safe("ls;"));
        assert!(safe("echo 'a;rm x'"));
        assert!(!safe(";"));
    }

    #[test]
    fn escaped_semicolon_find_exec_still_refused() {
        assert!(!safe(r#"find . -exec rm {} \;"#));
    }
}

/// True when mission text declares read-only intent, including negated forms
/// ("No edits.", "Do not modify"). Missions that declare mutation criteria
/// (WRITE_OK: etc.) are never read-only. "No edits to X" is a scoped carve-out,
/// not a read-only declaration, so negations followed by a scope word are ignored.
pub(crate) fn mission_declares_read_only(text: &str) -> bool {
    let t = text.to_lowercase();
    if ["write_ok:", "patch_function_ok:", "diff_patch_ok:", "replace_lines_ok:"]
        .iter()
        .any(|m| t.contains(m))
    {
        return false;
    }
    const DIRECT: &[&str] = &["read-only", "read only", "readonly", "strictly read"];
    const NEGATED: &[&str] = &[
        "no edits",
        "no edit",
        "do not edit",
        "don't edit",
        "do not modify",
        "don't modify",
        "no writes",
        "no write",
        "do not write",
        "don't write",
    ];
    const SCOPE_WORDS: &[&str] =
        &["to", "in", "on", "outside", "beyond", "except", "besides", "within", "for", "of"];

    let occurrences = |phrase: &str| -> Vec<usize> {
        let mut v = Vec::new();
        let mut from = 0;
        while let Some(i) = t[from..].find(phrase) {
            let start = from + i;
            let end = start + phrase.len();
            let before_ok = t[..start].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
            let after_ok = t[end..].chars().next().map_or(true, |c| !c.is_alphanumeric());
            if before_ok && after_ok {
                v.push(end);
            }
            from = end;
        }
        v
    };

    if DIRECT.iter().any(|p| !occurrences(p).is_empty()) {
        return true;
    }
    NEGATED.iter().any(|p| {
        occurrences(p).into_iter().any(|end| {
            let next_word: String =
                t[end..].trim_start().chars().take_while(|c| c.is_alphabetic()).collect();
            !SCOPE_WORDS.contains(&next_word.as_str())
        })
    })
}

/// True when `word` occurs at the start of a token (not preceded by a letter or
/// digit): matches "patch", "patch_function", "patching"; not "dispatcher".
pub(crate) fn contains_word_start(text: &str, word: &str) -> bool {
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let start = from + i;
        if text[..start].chars().next_back().map_or(true, |c| !c.is_alphanumeric()) {
            return true;
        }
        from = start + word.len();
    }
    false
}

#[cfg(test)]
mod mission_read_only_tests {
    use super::{contains_word_start, mission_declares_read_only as ro};

    #[test]
    fn a4_prompt_is_read_only() {
        assert!(ro("## Mission\nMap public surface of edge minion, terminal_core, and substrate. Stubs and empty mods only.\n## Files only\n1. crates/nexus_edge_minion/src/lib.rs\nOne file to 100%, then next. No fourth file. No execute_command. No edits."));
    }

    #[test]
    fn a2_a3_style_negations() {
        assert!(ro(
            "One file to 100%, then the other. No third file. No execute_command. No edits."
        ));
        assert!(ro("Read-only. No edits."));
        assert!(ro("Strictly read-only. No other commands."));
        assert!(ro("Do not modify any file."));
    }

    #[test]
    fn scoped_carve_outs_and_mutation_missions_are_not_read_only() {
        assert!(!ro("Patch function foo in a.rs. No edits to other files."));
        assert!(!ro("Fix the parser and add tests."));
        assert!(!ro(
            "Write the report. WRITE_OK: out/report.md. No edits elsewhere. Read-only otherwise."
        ));
        assert!(!ro("The reeditor and the proofreadonly crate"));
    }

    #[test]
    fn word_start_does_not_match_dispatch() {
        assert!(!contains_word_start("pub mod dispatcher; ", "patch"));
        assert!(!contains_word_start("task dispatch", "patch"));
        assert!(contains_word_start("patch the file", "patch"));
        assert!(contains_word_start("apply patch_function now", "patch"));
        assert!(contains_word_start("patching a.rs", "patch"));
    }
}
