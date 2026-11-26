use crate::auth;

pub fn write_startup_token() -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let token = auth::issue_sovereign_token()?;
    // ⚡ FIX: Strict 0600 permissions
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(".nexus_ui_token")?
        .write_all(token.as_bytes())?;

    println!("\x1b[32m>> [SECURITY] Startup token written to .nexus_ui_token (0600)\x1b[0m");
    Ok(())
}
