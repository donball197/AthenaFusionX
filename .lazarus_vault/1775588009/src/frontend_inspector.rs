use crate::fs_utils;
use anyhow::Result;

pub fn scan_ui_layer() -> Result<String> {
    let files_to_scan = vec![
        "ui/src/routes/+page.svelte",
        "ui/src/lib/components/Editor.svelte",
        "ui/src/lib/components/Terminal.svelte",
    ];

    println!(">> [INSPECTOR] Ingesting Svelte UI DNA...");
    let mut payload = String::new();

    for path in files_to_scan {
        let content = fs_utils::read_file_safe(path).unwrap_or_else(|_| "// FILE MISSING".into());
        payload.push_str(&format!("--- TARGET: {} ---\n{}\n\n", path, content));
    }

    Ok(payload)
}
