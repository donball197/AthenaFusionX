use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tokio::process::Command;
use blake3::Hasher;
use chrono::Utc;
use crate::neural_agent::ContextPacket;
use crate::server::AppState;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Clone, Serialize, Deserialize)]
pub struct Repair {
    pub hash: String,
    pub strat: String,
    pub ok: bool,
    pub err: String,
    pub ts: String,
}

pub struct AutoHealer {
    pub hist: HashMap<String, Vec<Repair>>,
    pub db_path: String,
}

impl AutoHealer {
    pub fn new() -> Self {
        Self::with_path(".nexus_heal.json")
    }

    pub fn with_path(path: &str) -> Self {
        let mut h = Self { 
            hist: HashMap::new(),
            db_path: path.to_string(),
        };
        let _ = h.load();
        h
    }

    pub fn load(&mut self) -> Result<()> {
        if let Ok(d) = fs::read_to_string(&self.db_path) {
            self.hist = serde_json::from_str(&d).unwrap_or_default();
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        fs::write(&self.db_path, serde_json::to_string_pretty(&self.hist)?)?;
        Ok(())
    }

    pub fn skip(&self, path: &str, strat: &str, err_snip: &str) -> bool {
        self.hist.get(path).map_or(false, |rs|
            rs.iter().any(|r| r.strat == strat && r.err == err_snip && !r.ok))
    }

    pub fn record(&mut self, path: &str, strat: &str, ok: bool, err_snip: &str) {
        let hash = fs::read(path).ok().map(|b| Hasher::new().update(&b).finalize().to_hex().to_string()).unwrap_or_default();
        let r = Repair {
            hash,
            strat: strat.into(),
            ok,
            err: err_snip.into(),
            ts: Utc::now().to_rfc3339(),
        };
        self.hist.entry(path.into()).or_default().push(r);
        if let Some(v) = self.hist.get_mut(path) { if v.len() > 40 { v.drain(0..v.len()-40); } }
        let _ = self.save();
    }

    pub async fn attempt_auto_repair(state: &Arc<AppState>, path: &str, err_log: &str) -> Result<(bool, String)> {
        let mut healer = Self::new();
        let snip = err_log.chars().take(280).collect::<String>();
        let mut rep = String::from("\n>> [HEURISTIC HEALER] Initiating Adaptive Repair...\n");

        if !healer.skip(path, "clippy", &snip) {
            rep.push_str("-> Strategy 0: clippy --fix\n");
            if let Ok(ok) = healer.clippy(path).await {
                healer.record(path, "clippy", ok, &snip);
                if ok { rep.push_str("   [SUCCESS] Clippy resolved issue.\n"); return Ok((true, rep)); }
            }
        }

        if !healer.skip(path, "fmt", &snip) {
            rep.push_str("-> Strategy 1: fmt + cargo check --fix\n");
            if let Ok(ok) = healer.fmt_check(path).await {
                healer.record(path, "fmt", ok, &snip);
                if ok { rep.push_str("   [SUCCESS] Format & Fix resolved issue.\n"); return Ok((true, rep)); }
            }
        }

        if !healer.skip(path, "llm", &snip) {
            rep.push_str("-> Strategy 2: Surgical LLM Patch (Tier 3)\n");
            if let Ok((ok, msg)) = healer.llm(state, path, err_log).await {
                if ok {
                    if let Ok(out) = Command::new("cargo").args(["check", "--quiet"]).output().await {
                        if out.status.success() {
                            healer.record(path, "llm", true, &snip);
                            rep.push_str(&format!("   [SUCCESS] {}. Patch compiles cleanly.\n", msg));
                            return Ok((true, rep));
                        } else {
                            healer.record(path, "llm", false, &snip);
                            rep.push_str("   [FAILED] LLM patch applied but failed to compile.\n");
                        }
                    }
                } else {
                    healer.record(path, "llm", false, &snip);
                    rep.push_str(&format!("   [FAILED] {}\n", msg));
                }
            }
        }

        rep.push_str("-> Strategy 3: Revert to HEAD\n");
        if let Ok(ok) = healer.revert(path).await {
            healer.record(path, "revert", ok, &snip);
            if ok { rep.push_str("   [SUCCESS] Reverted uncommitted changes.\n"); return Ok((true, rep)); }
        }

        rep.push_str("!! ALL STRATEGIES EXHAUSTED !!\n");
        Ok((false, rep))
    }

    async fn clippy(&self, _p: &str) -> Result<bool> {
        Ok(Command::new("cargo").args(["clippy","--fix","--allow-dirty","--allow-staged","-q","--","-Awarnings"]).status().await?.success())
    }

    async fn fmt_check(&self, p: &str) -> Result<bool> {
        let _ = Command::new("rustfmt").arg(p).status().await;
        Ok(Command::new("cargo").args(["check", "-q"]).status().await?.success())
    }

    async fn llm(&self, state: &Arc<AppState>, p: &str, log: &str) -> Result<(bool,String)> {
        let code = fs::read_to_string(p).unwrap_or_default();
        let prompt = format!("FIX_ERROR FILE:{} ERROR:{} CODE:```rust\n{}\n``` Output ONLY full file in ```rust``` block.", p, log.chars().take(2500).collect::<String>(), code);
        let ctx = ContextPacket { file_content: Some(code), terminal_log: Some(log.into()), ..Default::default() };
        let res_val = f32::from_bits(state.current_resonance.load(Ordering::Relaxed));
        
        let (r, _) = state.agent.prepare_and_run(&state.client, &state.api_key, &prompt, ctx, res_val).await?;
        
        if let Some(start) = r.find("```rust") {
            let body = &r[start + 7..];
            if let Some(end) = body.find("```") {
                let patch = body[..end].trim().to_string();
                if patch.len() > 10 {
                    fs::write(p, &patch)?;
                    return Ok((true, "Patched via LLM".into()));
                }
            }
        }
        Ok((false, "No valid markdown block returned".into()))
    }

    async fn revert(&self, p: &str) -> Result<bool> {
        Ok(Command::new("git").args(["checkout","HEAD","--",p]).status().await?.success())
    }
}

pub async fn background_heal_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            if let Ok(o) = Command::new("cargo").args(["check","-q"]).output().await {
                if !o.status.success() {
                    let err = String::from_utf8_lossy(&o.stderr).to_string();
                    let target_file = err.lines()
                        .find(|l| l.contains("--> "))
                        .and_then(|l| l.split("--> ").nth(1))
                        .and_then(|l| l.split(':').next())
                        .map(|s| s.trim())
                        .unwrap_or("src/main.rs");
                        
                    let _ = AutoHealer::attempt_auto_repair(&state, target_file, &err).await;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_skip_logic() {
        // Use a dummy path so it doesn't write to the real project file
        let mut healer = AutoHealer::with_path("dummy.json");
        healer.record("test.rs", "clippy", false, "borrow error");
        assert!(healer.skip("test.rs", "clippy", "borrow error"));
        assert!(!healer.skip("test.rs", "clippy", "other error"));
        assert!(!healer.skip("test.rs", "llm", "borrow error"));
    }

    #[test]
    fn test_history_persistence() -> anyhow::Result<()> {
        let dir = tempdir()?;
        // Create an explicit isolated path for this specific test
        let db_path = dir.path().join("heal.json");
        let db_str = db_path.to_str().unwrap();

        {
            let mut healer = AutoHealer::with_path(db_str);
            healer.record("a.rs", "fmt", true, "syntax");
        } // Healer drops, but `record` safely saves to the isolated path

        // Reboot the healer pointing to the exact same isolated path
        let mut healer_reboot = AutoHealer::with_path(db_str);

        assert_eq!(healer_reboot.hist["a.rs"][0].strat, "fmt");
        assert!(healer_reboot.hist["a.rs"][0].ok);

        Ok(())
    }

    #[test]
    fn test_file_hash_in_record() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("heal.json");
        let file_path = dir.path().join("f.rs");
        
        File::create(&file_path).unwrap().write_all(b"fn main() {}").unwrap();

        let mut healer = AutoHealer::with_path(db_path.to_str().unwrap());
        healer.record(file_path.to_str().unwrap(), "fmt", true, "syntax");

        let hash = &healer.hist[file_path.to_str().unwrap()][0].hash;
        assert_eq!(hash.len(), 64); 
    }
}
