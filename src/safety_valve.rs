use std::process::Command;

pub fn engage_safety_valve(file_path: &str) -> anyhow::Result<()> {
    println!(">> [VALVE] Securing perimeter for {}...", file_path);

    // 1. Ensure we are on the 'Lazarus' branch
    Command::new("git").args(["checkout", "-b", "lazarus"]).status().ok();

    // 2. Stage the current state
    Command::new("git").args(["add", file_path]).status()?;

    // 3. Commit with a Neural Timestamp
    let msg = format!("Lazarus Snapshot: Pre-Refactor of {}", file_path);
    Command::new("git").args(["commit", "-m", &msg]).status()?;

    println!(">> [VALVE] Snapshot committed. Neural Drift contained.");
    Ok(())
}
