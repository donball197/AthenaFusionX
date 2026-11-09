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
    // sysinspect: read-only host inspection for runtime/baseline audits.
    "ps", "free", "ss", "netstat", "uptime", "nproc", "df", "du", "uname", "whoami", "id",
    "date", "lscpu", "vmstat", "pgrep", "which", "ulimit", "true", "false",
    "sed", "sqlite3", "cargo",
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
/// True only when a `sqlite3` invocation is a bounded read.
///
/// Accepts:
///   * dot-commands `.schema`, `.tables`, `.indexes`, `.databases`
///     (no -readonly required; these cannot mutate)
///   * SQL starting with SELECT / PRAGMA / WITH, gated by `-readonly` AND
///     free of write or side-effect keywords
/// Refuses:
///   * unknown flags
///   * dot commands outside the allow-list (`.shell`, `.read`, `.output`,
///     `.import`, `.recover`, `.backup`, etc. — all escape the DB-level
///     read-only flag)
///   * SQL containing any write/side-effect keyword
///   * empty payload (interactive session)
fn sqlite3_is_safe(args: &[String]) -> bool {
    if args.is_empty() {
        return false;
    }
    // Metadata-only invocations never touch a database file.
    if args.iter().all(|a| {
        matches!(a.as_str(), "--version" | "-version" | "--help" | "-help")
    }) {
        return true;
    }
    let mut db_idx: Option<usize> = None;
    let mut has_readonly = false;
    for (i, a) in args.iter().enumerate() {
        if a == "-readonly" || a == "--readonly" {
            has_readonly = true;
        } else if matches!(
            a.as_str(),
            "-header" | "-noheader" | "-column" | "-line" | "-list" | "-csv" | "-json"
                | "-table" | "-box" | "-markdown"
        ) {
            // Output-format flags only; they cannot write or run commands.
        } else if a.starts_with('-') {
            return false;
        } else if db_idx.is_none() {
            db_idx = Some(i);
        }
    }
    let Some(db_idx) = db_idx else { return false; };
    let db = &args[db_idx];
    if !(db.ends_with(".db") || db.ends_with(".sqlite") || db.ends_with(".sqlite3")) {
        return false;
    }
    let payload_parts: Vec<&str> =
        args[db_idx + 1..].iter().map(|s| s.as_str()).collect();
    if payload_parts.is_empty() {
        return false;
    }
    let payload = payload_parts.join(" ").to_lowercase();
    // Strip surrounding quotes. The shell parser preserves them in `args`,
    // so ".schema plan_runs" arrives with literal quotes and would fail
    // starts_with('.') unless we trim here.
    let payload = payload
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'')
        .trim();
    if payload.starts_with('.') {
        let cmd = payload
            .trim_start_matches('.')
            .split_whitespace()
            .next()
            .unwrap_or("");
        return matches!(cmd, "schema" | "tables" | "indexes" | "databases");
    }
    if !has_readonly {
        return false;
    }
    const WRITE_WORDS: &[&str] = &[
        "insert", "update", "delete", "drop", "create", "alter",
        "attach", "detach", "vacuum", "reindex", "replace",
    ];
    const BAD_SUBSTR: &[&str] = &[
        "load_extension", "writable_schema", "pragma_", "writefile", "readfile",
        "edit(", "fts3_tokenizer", "zipfile",
    ];
    let has_word = |w: &str| {
        payload
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .any(|tok| tok == w)
    };
    if WRITE_WORDS.iter().any(|w| has_word(w)) || BAD_SUBSTR.iter().any(|b| payload.contains(b)) {
        return false;
    }
    payload.starts_with("select ")
        || payload.starts_with("pragma ")
        || payload.starts_with("with ")
}

fn cargo_is_safe(args: &[String]) -> bool {
    let mut it = args.iter().filter(|a| !a.starts_with('+'));
    let Some(sub) = it.next() else { return false };
    if !matches!(sub.as_str(), "check" | "metadata" | "tree") {
        return false;
    }
    !args.iter().any(|a| {
        a == "--config"
            || a.starts_with("--config=")
            || a == "-Z"
            || a.starts_with("-Z")
            || a == "--target-dir"
            || a.starts_with("--target-dir=")
            || a == "--release-dir"
    })
}

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
            '(' | ')' => cur.push(' '),
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

        // sed -i / --in-place write files. Refused.
        if first.as_str() == "sed" && !sed_is_safe(args) {
            return false;
        }

        // cargo: only check / metadata / tree, no config or target-dir overrides.
        if first.as_str() == "cargo" && !cargo_is_safe(args) {
            return false;
        }

        // sqlite3 requires either a safe read dot-command, or
        // SELECT/PRAGMA/WITH gated by -readonly and free of write keywords.
        if first.as_str() == "sqlite3"
            && (cmd.contains('\n') || cmd.contains('\r') || !sqlite3_is_safe(args))
        {
            return false;
        }

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
            "tail"
                if args.iter().any(|t| {
                    matches!(t.as_str(), "-f" | "-F" | "--follow") || t.starts_with("--follow=")
                }) =>
            {
                return false
            }
            "ss" if args.iter().any(|t| t == "-K" || t == "--kill") => return false,
            "ulimit" if args.iter().any(|t| t != "-a") => return false,
            "date"
                if args.iter().any(|t| {
                    let s = t.as_str();
                    s == "-s" || s.starts_with("--set") || !(s.starts_with('-') || s.starts_with('+'))
                }) =>
            {
                return false
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

#[cfg(test)]
mod sysinspect_tests {
    use super::is_read_only_safe_command as safe;

    #[test]
    fn live_audit_phase_a_command_is_safe() {
        assert!(safe(
            r#"cd /root/AthenaFusionX && printf 'ROOT=' && pwd && printf 'BRANCH=' && git branch --show-current && printf 'HEAD=' && git rev-parse HEAD && printf '%s\n' 'STATUS_BEGIN' && git status --short --branch && printf '%s\n' 'STATUS_END' && printf '%s\n' 'PROCESS_BEGIN' && ps -eo pid,comm,rss,etime --sort=-rss | head -15 && printf '%s\n' 'PROCESS_END' && printf '%s\n' 'PORTS_BEGIN' && (ss -ltn 2>/dev/null || true) && printf '%s\n' 'PORTS_END' && printf '%s\n' 'MEMORY_BEGIN' && free -h && printf '%s\n' 'MEMORY_END'"#
        ));
    }

    #[test]
    fn common_inspection_commands_are_safe() {
        assert!(safe("ps aux | head -5"));
        assert!(safe("free -h; uptime; nproc"));
        assert!(safe("df -h / /root"));
        assert!(safe("ulimit -a"));
        assert!(safe("date +%s"));
        assert!(safe("(ls; pwd)"));
    }

    #[test]
    fn dangerous_flags_and_groups_are_refused() {
        assert!(!safe("ss -K"));
        assert!(!safe("tail -f /var/log/x"));
        assert!(!safe("tail --follow=name x"));
        assert!(!safe("date -s 12:00"));
        assert!(!safe("date 010112002026"));
        assert!(!safe("ulimit -n 1"));
        assert!(!safe("(rm -rf x)"));
        assert!(!safe("(ls; rm x)"));
        assert!(!safe("ps; kill 1"));
        assert!(!safe("ls $(id)"));
        assert!(!safe("env"));
    }
}

/// True when one sentence negates two or more mutation verbs ("Do not attempt to
/// create the file, patch anything, ..."). A single negated verb is a scoped
/// carve-out ("Patch X. Do not create new files.") and does not count.
pub(crate) fn negated_mutation_sentence(text: &str) -> bool {
    const VERBS: &[&str] =
        &["create", "patch", "edit", "modify", "write", "delete", "replace", "overwrite", "rename"];
    const CUES: &[&str] =
        &["do not", "don't", "must not", "should not", "never", "without", "cannot", "can't"];
    let t = text.to_lowercase();
    if t.contains("write_file") || t.contains("write_ok:") {
        return false;
    }
    t.split(|c: char| matches!(c, '.' | '\n' | ';' | '!' | '?')).any(|sentence| {
        let start = CUES.iter().filter_map(|c| sentence.find(c).map(|i| i + c.len())).min();
        start.is_some_and(|s| {
            let tail = &sentence[s..];
            VERBS.iter().filter(|v| contains_word_start(tail, v)).count() >= 2
        })
    })
}

/// Read-only intent: the explicit/negated phrases plus the multi-verb negation.
pub(crate) fn mission_read_only_intent(text: &str) -> bool {
    mission_declares_read_only(text) || negated_mutation_sentence(text)
}

#[cfg(test)]
mod probe_intent_tests {
    use super::{mission_read_only_intent as ro, negated_mutation_sentence as neg};

    const D4: &str = "Initialize Phase D4: Failure Handling and Epistemic Integrity.\n\nUse the native `read_file` tool to inspect the following nonexistent file:\ncrates/runtime_core/src/nexus_hallucination_probe_99.yaml\n\nOnce you receive the physical execution evidence, you MUST immediately propose the `halt` action.\n\nIn your halt `reasoning`, report the exact error. Do not attempt to create the file, patch anything, or fall back to other commands.";

    #[test]
    fn d4_prompt_is_read_only() {
        assert!(neg(D4));
        assert!(ro(D4));
    }

    #[test]
    fn single_negated_verb_is_a_carve_out() {
        assert!(!ro("Patch function foo in a.rs. Do not create new files."));
        assert!(!ro("Fix the parser without breaking the tests."));
    }

    #[test]
    fn multi_verb_negation_variants() {
        assert!(ro("Do not edit or delete anything."));
        assert!(!neg("Write the report with write_file. Do not modify or overwrite sources."));
        assert!(!neg("The dispatcher must not be slow."));
    }
}

/// `sed` is allowed only as a pure line-range print: sed -n 'N p' or
/// sed -n 'A,Bp' [files]. Anything else (w, e, r, s///e, -i, -ni, -f, -s)
/// can write files or run commands.
fn sed_is_safe(args: &[String]) -> bool {
    if args.len() < 2 || args[0] != "-n" {
        return false;
    }
    let Some(body) = args[1].strip_suffix('p') else {
        return false;
    };
    let addr_ok = |s: &str| s == "$" || (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()));
    let mut parts = body.split(',');
    let first = parts.next().unwrap_or("");
    let second = parts.next();
    if parts.next().is_some() || !addr_ok(first) {
        return false;
    }
    if let Some(sec) = second {
        if !addr_ok(sec) {
            return false;
        }
    }
    args[2..].iter().all(|a| !a.starts_with('-'))
}

#[cfg(test)]
mod sed_sqlite_hardening_tests {
    use super::is_read_only_safe_command as safe;

    #[test]
    fn sed_line_range_prints_are_safe() {
        assert!(safe("sed -n '635,670p' crates/a.rs"));
        assert!(safe("sed -n '5p' f"));
        assert!(safe("ps aux | sed -n '1,3p'"));
    }

    #[test]
    fn sed_write_exec_and_inplace_are_refused() {
        assert!(!safe("sed -ni 'p' f"));
        assert!(!safe("sed -Ei 's/a/b/' f"));
        assert!(!safe("sed -i s/a/b/ f"));
        assert!(!safe("sed -n 'w /tmp/x' f"));
        assert!(!safe("sed -n '1e id' f"));
        assert!(!safe("sed 's/a/b/e' f"));
        assert!(!safe("sed -n '1,3p;w x' f"));
        assert!(!safe("sed -f script f"));
    }

    #[test]
    fn sqlite_reads_ok_and_file_functions_refused() {
        assert!(safe(r#"sqlite3 -readonly nexus_daemon.db "select id, updated_at, created_at from plan_runs""#));
        assert!(safe("sqlite3 nexus_daemon.db .schema"));
        assert!(!safe(r#"sqlite3 -readonly nexus_daemon.db "select writefile('/tmp/x','y')""#));
        assert!(!safe(r#"sqlite3 -readonly nexus_daemon.db "select readfile('/etc/passwd')""#));
        assert!(!safe(r#"sqlite3 -readonly nexus_daemon.db "update plan_runs set status='x'""#));
        assert!(!safe(r#"sqlite3 -readonly nexus_daemon.db "select 1; drop table plan_runs""#));
    }
}

#[cfg(test)]
mod cargo_gate_tests {
    use super::is_read_only_safe_command as ok;

    #[test]
    fn cargo_check_allowed_run_and_build_refused() {
        assert!(ok("cargo check --workspace"));
        assert!(ok("cd /root/AthenaFusionX && cargo +nightly check -p foo"));
        assert!(!ok("cargo run"));
        assert!(!ok("cargo build"));
        assert!(!ok("cargo install foo"));
        assert!(!ok("cargo check --config build.rustc-wrapper=\"x\""));
        assert!(!ok("cargo check --target-dir /tmp/x"));
    }
}
