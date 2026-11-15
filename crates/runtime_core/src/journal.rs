//! Append-only, hash-chained per-turn journal for deterministic replay.
//!
//! Each record seals a SHA-256 over its own fields plus the previous record's
//! hash, so tampering with any record invalidates every later record. The
//! journal is JSONL (one sealed record per line) and never truncates.
//!
//! # Replay portability (design note — not yet implemented)
//!
//! `paths_equivalent` (evidence_paths.rs) bridges absolute/relative paths
//! by stripping the *current* working directory at verify time. A journal
//! written under /root/AthenaFusionX therefore will not verify under
//! /home/don/AthenaFusionX: recorded `[WRITE_OK] /root/...` fails the
//! strip_prefix(cwd) bridge and falls back to raw comparison against the
//! contract's relative path.
//!
//! When a replay tool is built, add `workspace_root: String` to TurnRecord
//! (serde-default to "" for backwards compat with pre-2026-11 records),
//! populate it in `append_global` from `std::env::current_dir()`, and
//! rewrite absolute paths under the recorded root to their relative form
//! in the replay loader before feeding evidence to the verifier.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub node_id: String,
    pub turn: u32,
    pub strategy: String,
    pub action_type: String,
    /// SHA-256 of the physical evidence bytes produced this turn.
    pub evidence_hash: String,
    pub evidence_len: usize,
    pub belief_before: f32,
    pub belief_after: f32,
    pub eval_modifier: f32,
    pub unmet_before: Vec<String>,
    pub unmet_after: Vec<String>,
    pub provider: Option<String>,
    pub latency_ms: Option<u64>,
    /// Hash of the previous record; genesis is 64 zeros.
    pub prev_hash: String,
    pub record_hash: String,
}

pub fn hash_bytes(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

impl TurnRecord {
    /// Recompute `record_hash` over the immutable fields plus `prev_hash`.
    /// Call after populating every other field and before appending.
    pub fn seal(&mut self) {
        let mut h = Sha256::new();
        h.update(self.node_id.as_bytes());
        h.update(self.turn.to_le_bytes());
        h.update(self.strategy.as_bytes());
        h.update(self.action_type.as_bytes());
        h.update(self.evidence_hash.as_bytes());
        h.update(self.belief_before.to_le_bytes());
        h.update(self.belief_after.to_le_bytes());
        h.update(self.eval_modifier.to_le_bytes());
        for u in &self.unmet_before {
            h.update(u.as_bytes());
        }
        h.update(b"|");
        for u in &self.unmet_after {
            h.update(u.as_bytes());
        }
        h.update(self.prev_hash.as_bytes());
        self.record_hash = format!("{:x}", h.finalize());
    }
}

pub struct Journal {
    path: std::path::PathBuf,
    prev_hash: String,
    turn: u32,
}

impl Journal {
    pub fn open(path: impl Into<std::path::PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let mut prev = "0".repeat(64);
        let mut turn = 0u32;
        if let Ok(body) = std::fs::read_to_string(&path) {
            let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
            turn = lines.len() as u32;
            if let Some(last) = lines.last() {
                if let Ok(rec) = serde_json::from_str::<TurnRecord>(last) {
                    prev = rec.record_hash;
                }
            }
        }
        Ok(Self { path, prev_hash: prev, turn })
    }

    pub fn append(&mut self, mut rec: TurnRecord) -> std::io::Result<()> {
        use std::io::Write;
        if rec.turn == 0 {
            self.turn += 1;
            rec.turn = self.turn;
        }
        rec.prev_hash = self.prev_hash.clone();
        rec.seal();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(&rec)
                .map_err(|e| { std::io::Error::new(std::io::ErrorKind::Other, e) })?
        )?;
        self.prev_hash = rec.record_hash.clone();
        Ok(())
    }

    /// Verify the chain. Returns the first broken line index, if any.
    pub fn verify_chain(body: &str) -> Option<usize> {
        let mut prev = "0".repeat(64);
        for (i, line) in body.lines().enumerate() {
            let mut rec: TurnRecord = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(_) => return Some(i),
            };
            if rec.prev_hash != prev {
                return Some(i);
            }
            let declared = rec.record_hash.clone();
            rec.seal();
            if rec.record_hash != declared {
                return Some(i);
            }
            prev = declared;
        }
        None
    }
}

// --- process-wide singleton + fire-and-forget append ------------------

pub fn global() -> &'static std::sync::Mutex<Option<Journal>> {
    static J: std::sync::OnceLock<std::sync::Mutex<Option<Journal>>> = std::sync::OnceLock::new();
    J.get_or_init(|| std::sync::Mutex::new(None))
}

/// Append one turn record to the process-wide journal. No-op when the
/// journal cannot be opened (read-only missions, unwritable .nexus_debug).
pub fn append_global(rec: TurnRecord) {
    let Ok(mut g) = global().lock() else { return };
    if g.is_none() {
        let _ = std::fs::create_dir_all(".nexus_debug");
        *g = Journal::open(".nexus_debug/journal.jsonl").ok();
    }
    if let Some(j) = g.as_mut() {
        let _ = j.append(rec);
    }
}

/// Build a journal record for a critic-side event (an override or a deterministic
/// gate rejection). `action_type` is the event label, e.g. "critic_override:write_file".
/// The first 200 chars of `detail` ride in `unmet_before` so the audit trail is readable.
pub fn critic_event_record(
    action_type: &str,
    strategy: &str,
    action_key: &str,
    modifier: f32,
    detail: &str,
) -> TurnRecord {
    let excerpt: String = detail.replace('\n', " ").chars().take(200).collect();
    TurnRecord {
        node_id: "critic".to_string(),
        turn: 0,
        strategy: strategy.to_string(),
        action_type: action_type.to_string(),
        evidence_hash: hash_bytes(excerpt.as_bytes()),
        evidence_len: excerpt.len(),
        belief_before: 0.0,
        belief_after: 0.0,
        eval_modifier: modifier,
        unmet_before: vec![format!("detail: {excerpt}")],
        unmet_after: vec![format!("action: {action_key}")],
        provider: None,
        latency_ms: None,
        prev_hash: String::new(),
        record_hash: String::new(),
    }
}

/// Fire-and-forget journal entry for a critic override or gate rejection.
pub fn append_critic_event(
    action_type: &str,
    strategy: &str,
    action_key: &str,
    modifier: f32,
    detail: &str,
) {
    append_global(critic_event_record(action_type, strategy, action_key, modifier, detail));
}

#[cfg(test)]
mod critic_event_tests {
    use super::*;

    #[test]
    fn builds_a_sealable_record_with_truncated_detail() {
        let long = "x".repeat(500);
        let mut r = critic_event_record("critic_override:write_file", "write_file:a.md", "k1", -0.4, &long);
        assert_eq!(r.action_type, "critic_override:write_file");
        assert_eq!(r.eval_modifier, -0.4);
        assert_eq!(r.evidence_len, 200);
        assert!(r.unmet_before[0].starts_with("detail: "));
        assert_eq!(r.unmet_after[0], "action: k1");
        r.seal();
        assert_eq!(r.record_hash.len(), 64);
    }

    #[test]
    fn newlines_in_detail_are_flattened() {
        let r = critic_event_record("critic_gate:forbidden_removal", "s", "k", 0.0, "a\nb");
        assert_eq!(r.unmet_before[0], "detail: a b");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(turn: u32, prev: &str) -> TurnRecord {
        TurnRecord {
            node_id: "n1".into(),
            turn,
            strategy: "read_file:x.rs".into(),
            action_type: "read_file".into(),
            evidence_hash: hash_bytes(b"ev"),
            evidence_len: 2,
            belief_before: 0.5,
            belief_after: 0.7,
            eval_modifier: 0.2,
            unmet_before: vec!["ReadMarkerAcquired".into()],
            unmet_after: vec![],
            provider: None,
            latency_ms: None,
            prev_hash: prev.into(),
            record_hash: String::new(),
        }
    }

    #[test]
    fn chain_roundtrips() {
        let mut a = rec(1, &"0".repeat(64));
        a.seal();
        let mut b = rec(2, &a.record_hash);
        b.seal();
        let body = format!(
            "{}\n{}\n",
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
        assert_eq!(Journal::verify_chain(&body), None);
    }

    #[test]
    fn tamper_breaks_chain() {
        let mut a = rec(1, &"0".repeat(64));
        a.seal();
        let body = serde_json::to_string(&a).unwrap().replace("0.7", "0.9");
        assert_eq!(Journal::verify_chain(&body), Some(0));
    }
}
