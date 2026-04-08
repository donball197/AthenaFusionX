use chrono::Utc;
use anyhow::Result;

pub fn log_resonance_time() -> Result<()> {
    let now = Utc::now();
    println!("\x1b[38;5;240m>> [ZENITH_PULSE] Resonance synced at: {}\x1b[0m", now.to_rfc3339());
    Ok(())
}
