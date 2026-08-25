//! End-to-end tests for the auto-heal pipeline.
//! All tests are #[ignore] — they need API keys + network.
//! Run with: cargo test -p nexus_runtime_core --test e2e_auto_heal -- --ignored --nocapture

use std::path::Path;
use std::process::Command;

fn run_ast_tool(args: &[&str]) -> bool {
    let status = Command::new("cargo")
        .args(["run", "--bin", "nexus_ast_tool", "--"])
        .args(args)
        .status()
        .expect("Failed to execute nexus_ast_tool");
    status.success()
}

#[test]
#[ignore = "needs GEMINI/NVIDIA keys + network"]
fn test_php_sqli_auto_heal() {
    let fixture_dir = Path::new("tests/fixtures/sqli_php");
    assert!(fixture_dir.exists(), "SQLi fixture directory missing");
    let target_file = fixture_dir.join("www/login.php");
    let original = std::fs::read_to_string(&target_file).unwrap();

    let success = run_ast_tool(&[
        "audit", "--path", fixture_dir.to_str().unwrap(),
        "--ai-triage", "--auto-heal", "--generate-report",
    ]);
    assert!(success, "nexus_ast_tool audit failed");

    let patched = std::fs::read_to_string(&target_file).unwrap_or_default();
    assert!(
        patched.contains("mysqli_prepare") || patched.contains("mysqli_stmt_bind_param"),
        "File was not patched with prepared statements. Content:\n{}", patched
    );
    std::fs::write(&target_file, original).ok();
}

#[test]
#[ignore = "needs GEMINI/NVIDIA keys + network"]
fn test_python_cmdi_auto_heal() {
    let fixture_dir = Path::new("tests/fixtures/cmdi_python");
    assert!(fixture_dir.exists(), "cmdi fixture directory missing");
    let target_file = fixture_dir.join("server.py");
    let original = std::fs::read_to_string(&target_file).unwrap();

    let success = run_ast_tool(&[
        "audit", "--path", fixture_dir.to_str().unwrap(),
        "--ai-triage", "--auto-heal", "--generate-report",
    ]);
    assert!(success, "nexus_ast_tool audit failed");

    let patched = std::fs::read_to_string(&target_file).unwrap_or_default();
    assert!(
        patched.contains("subprocess.run")
            && !patched.contains("os.popen")
            && !patched.contains("os.system"),
        "File was not patched with safe subprocess. Content:\n{}", patched
    );
    std::fs::write(&target_file, original).ok();
}
