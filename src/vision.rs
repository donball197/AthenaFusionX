use anyhow::Result;
use std::process::Command;
use std::fs;

pub async fn capture_frame() -> Result<String> {
    let path = "/sdcard/vision_input.jpg";
    println!(">> [EYE] Mobilizing Termux Eye...");
    
    let status = Command::new("bash")
        .arg("fetch_frame.sh")
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("Hardware blocked capture."));
    }

    // Direct File Reporting (Replaces the second session)
    if let Ok(metadata) = fs::metadata(path) {
        let size_mb = metadata.len() as f64 / 1_048_576.0;
        println!(">> [EYE] Frame captured: {:.2} MB seated at {}", size_mb, path);
    }

    Ok(path.to_string())
}
