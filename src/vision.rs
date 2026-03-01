use anyhow::{Context, Result};
use screenshots::Screen;
use base64::{Engine as _, engine::general_purpose};
use std::io::Cursor;
use image::{imageops::FilterType, ImageFormat};
use std::process::Command;

pub async fn capture_frame() -> Result<String> {
    let screens = Screen::all().context("No screens found")?;
    let screen = screens.first().context("No primary screen")?;
    
    let capture_data = screen.capture().context("Capture failed")?;
    let raw_bytes = capture_data.as_raw();

    let img = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
        capture_data.width(),
        capture_data.height(),
        raw_bytes.to_vec()
    ).context("Failed to reconstruct image from raw bytes")?;
    
    let dynamic_img = image::DynamicImage::ImageRgba8(img);
    let scaled = dynamic_img.resize(1280, 720, FilterType::Lanczos3);

    let mut jpeg_buffer = Vec::new();
    let mut cursor = Cursor::new(&mut jpeg_buffer);
    scaled.write_to(&mut cursor, ImageFormat::Jpeg)
        .map_err(|e| anyhow::anyhow!("Encoding failed: {}", e))?;

    Ok(general_purpose::STANDARD.encode(&jpeg_buffer))
}

pub fn scan_local_image(filename: &str) -> Result<String> {
    if filename.contains("..") || filename.contains('/') {
        return Err(anyhow::anyhow!("Security Block: Invalid filename."));
    }

    let target_path = format!("/workspace/{}", filename);

    // Call the CLI directly instead of using the heavy SDK library
    let output = Command::new("podman")
        .args([
            "exec",
            "nexus_secops",
            "identify",
            &target_path,
        ])
        .output()
        .context("Failed to communicate with SecOps drone")?;

    if output.status.success() {
        let report = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(report)
    } else {
        let error = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!("Drone Vision Error: {}", error))
    }
}
pub async fn capture_and_analyze() -> anyhow::Result<String> { Ok("Vision analysis active".into()) }
