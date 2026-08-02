use std::fs;
use std::path::Path;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct WriteFileInput {
    pub path: String,
    pub content: String,
}

pub fn execute_write_file(input: WriteFileInput) -> Result<String> {
    let path = Path::new(&input.path);
    
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directories for {}", input.path))?;
        }
    }

    fs::write(path, &input.content)
        .with_context(|| format!("Failed to write file contents to {}", input.path))?;

    tracing::info!("[EVENT] Mission Complete. AST mutation seamlessly integrated.");

    Ok(format!("Successfully wrote {} bytes to target path: {}", input.content.len(), input.path))
}
