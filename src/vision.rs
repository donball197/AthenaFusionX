use anyhow::{Context, Result};
use screenshots::Screen;
use base64::{Engine as _, engine::general_purpose};
use std::io::Cursor;
use image::{imageops::FilterType, ImageFormat};

pub async fn capture_frame() -> Result<String> {
    let screens = Screen::all().context("No screens found")?;
    let screen = screens.first().context("No primary screen")?;
    
    // Capture the screen
    let capture_data = screen.capture().context("Capture failed")?;
    
    // Access the raw bytes of the ImageBuffer directly.
    // screenshots::image::ImageBuffer implements Deref to [u8].
    let raw_bytes = capture_data.as_raw();

    // 1. Load from the raw byte memory
    // Since we know screenshots returns RGBA, we create the image from raw parts
    let img = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
        capture_data.width(),
        capture_data.height(),
        raw_bytes.to_vec()
    ).context("Failed to reconstruct image from raw bytes")?;
    
    let dynamic_img = image::DynamicImage::ImageRgba8(img);

    // 2. Downscale (Smart Vision optimization)
    let scaled = dynamic_img.resize(1280, 720, FilterType::Lanczos3);

    let mut jpeg_buffer = Vec::new();
    let mut cursor = Cursor::new(&mut jpeg_buffer);

    // 3. Write as JPEG
    scaled.write_to(&mut cursor, ImageFormat::Jpeg)
        .map_err(|e| anyhow::anyhow!("Encoding failed: {}", e))?;

    Ok(general_purpose::STANDARD.encode(&jpeg_buffer))
}
