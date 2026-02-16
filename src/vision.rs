use anyhow::{Context, Result};
use screenshots::Screen;
use image::imageops::FilterType;
use std::io::Cursor;
use base64::{Engine as _, engine::general_purpose};
use std::time::Instant;

/// Captures the screen, resizes it for AI consumption, and returns a Base64 string.
pub async fn capture_frame() -> Result<String> {
    let start = Instant::now();

    // 1. Detect Screens
    let screens = Screen::all().context("Failed to detect displays")?;
    if screens.is_empty() {
        return Err(anyhow::anyhow!("No displays found (Headless mode?)"));
    }

    // 2. Capture Primary Screen (Index 0)
    let screen = screens[0];
    let image = screen.capture().context("Failed to capture screenshot")?;

    // 3. Convert to DynamicImage for processing
    // The capture is RGBA, so we load it accordingly
    let dynamic_image = image::DynamicImage::ImageRgba8(image);

    // 4. Resize (Optimization for 4GB RAM)
    // We scale down to width 1024, preserving aspect ratio.
    // FilterType::Nearest is fastest and uses least CPU/RAM.
    let resized = dynamic_image.resize(1024, 768, FilterType::Nearest);

    // 5. Encode to JPG in memory (Lighter than PNG)
    let mut buffer = Cursor::new(Vec::new());
    resized.write_to(&mut buffer, image::ImageOutputFormat::Jpeg(70))
        .context("Failed to compress image")?;

    // 6. Convert to Base64
    let b64 = general_purpose::STANDARD.encode(buffer.get_ref());

    println!(">> [VISION] Frame captured & compressed in {:.2?}", start.elapsed());
    
    Ok(b64)
}

/// Simple motion detection stub (Placeholder for Phase 3)
pub fn get_motion_delta() -> f32 {
    // In the future, this will compare the previous screenshot 
    // to the current one to decide if Nexus needs to "look" again.
    0.0
}
