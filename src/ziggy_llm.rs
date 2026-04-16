use anyhow::{anyhow, Result};
use std::fs;
use std::process::Command;
use tokio::sync::Mutex;

// RAM SHIELD: Prevents OOM crashes on the 4GB Duet by enforcing strict sequential compilation.
static ZIGGY_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn execute_native_zig(code: &str) -> Result<String> {
    let _guard = ZIGGY_LOCK.lock().await;

    let path = format!("/tmp/ziggy_runtime_{}.zig", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs());
    fs::write(&path, code)?;

    let output = Command::new("zig").args(["run", "--optimize", "ReleaseFast", &path]).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(anyhow!("ZIG_FAULT: {}", String::from_utf8_lossy(&output.stderr)))
    }
}
