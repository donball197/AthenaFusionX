use crate::completion_engine::extract_required_command;

#[test]
fn tool_name_is_not_the_introducer_and_payload_quotes_are_not_the_command() {
    let objective = "Using ONLY the execute_command tool (do NOT use write_file), run exactly this shell command to setup the directory and file: mkdir -p /apps && echo 'fn add(a: i32, b: i32) -> i32 { 0 }' > /apps/calculator.rs";
    assert_eq!(
        extract_required_command(objective).as_deref(),
        Some("mkdir -p /apps && echo 'fn add(a: i32, b: i32) -> i32 { 0 }' > /apps/calculator.rs")
    );
}

#[test]
fn json_payload_quotes_are_not_the_command() {
    let objective = r#"Using execute_command, run exactly this command: echo '{"secret_token": "ATHENA_777_OMEGA"}' > secret.json"#;
    assert_eq!(
        extract_required_command(objective).as_deref(),
        Some(r#"echo '{"secret_token": "ATHENA_777_OMEGA"}' > secret.json"#)
    );
}

#[test]
fn backtick_form_still_works_with_tool_name_present() {
    let objective = "Using ONLY the execute_command tool, run this command: `rustc calculator.rs`";
    assert_eq!(extract_required_command(objective).as_deref(), Some("rustc calculator.rs"));
}

#[test]
fn quoted_command_immediately_after_the_word() {
    assert_eq!(
        extract_required_command(r#"Run the command "uptime" now"#).as_deref(),
        Some("uptime")
    );
    assert_eq!(extract_required_command("Execute command: 'uptime'").as_deref(), Some("uptime"));
}

#[test]
fn direct_binary_form_is_unchanged() {
    let got = extract_required_command("Execute cargo check -p math_utils to verify the build");
    assert!(got.is_some_and(|c| c.starts_with("cargo check -p math_utils")));
}

#[test]
fn non_ascii_objective_does_not_panic() {
    let _ = extract_required_command("Überprüfe das command: ls -la");
}

#[test]
fn distant_directive_colon_is_not_a_command() {
    let objective = "MATHEMATICAL_EXECUTION_VERIFIED: Execute `python3 -c 'print(1)'` to deterministically calculate and verify every mathematical result required by the following directive. The command must print the complete calculated results, including the full ALL and ANY tables, commutativity, idempotence, absorption, ordering, all 36 duality checks, and edge-case checks. Use the command execution output as the physical evidence for this node. DIRECTIVE: Verify the six-state ALL/ANY algebra.";
    assert_eq!(extract_required_command(objective).as_deref(), Some("python3 -c 'print(1)'"));
}

#[test]
fn execute_exactly_cuts_trailing_record_clause() {
    let o = "Execute exactly: cargo check --workspace, and record compilation success or failure, all compiler warnings, compiler errors, affected packages, and source locations.";
    assert_eq!(
        crate::completion_engine::extract_required_command(o).as_deref(),
        Some("cargo check --workspace")
    );
}

#[test]
fn quoted_comma_and_is_not_a_stopper() {
    let o = "Run exactly: grep -c \"a, and b\" file.txt";
    assert_eq!(
        crate::completion_engine::extract_required_command(o).as_deref(),
        Some("grep -c \"a, and b\" file.txt")
    );
}

#[test]
fn bare_command_matches_evidence_with_cd_prefix_and_stderr_redirect() {
    // strip_workspace_cd_prefix only strips `cd <target> &&` when the target
    // canonicalizes to the process cwd. `cargo test -p nexus_runtime_core`
    // runs from the package directory, so a hardcoded workspace path never
    // matches and the strip is a no-op — the test would fail regardless of
    // what cmd_match.rs did. Build the evidence from the actual cwd so the
    // strip fires and the production shape is exercised.
    let cwd = std::env::current_dir().expect("test cwd");
    let ev = format!(
        "[CMD_OK] Command: cd {} && cargo check --workspace 2>&1\nStatus: exit status: 0\n",
        cwd.display()
    );
    assert!(crate::cmd_match::command_condition_met("cargo check --workspace", &ev));
}

#[test]
fn execute_exactly_cuts_trailing_period_record_clause() {
    let o = "Execute exactly: cargo check --workspace. Record whether compilation succeeds or fails, all compiler warnings or errors, affected packages and source locations.";
    assert_eq!(
        crate::completion_engine::extract_required_command(o).as_deref(),
        Some("cargo check --workspace")
    );
}
