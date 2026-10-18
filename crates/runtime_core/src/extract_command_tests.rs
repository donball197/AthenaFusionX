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
