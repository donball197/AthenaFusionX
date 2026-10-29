use std::process::Command;

#[test]
fn test_compile_legitimate_write() {
    let output = Command::new("rustc")
        .arg("legitimate_write.txt")
        .output()
        .expect("failed to execute process");

    println!("Exit code: {:?}", output.status.code());
    println!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
}