// crates/runtime_core/src/evidence_ledger.rs  (new)

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceEvent {
    // ── Physical actions ──────────────────────────────────────────────
    ReadOk {
        path: String,
        bytes: usize,
        truncated: bool,
        /// Optional coverage range that was actually acquired
        range: Option<(u32, u32)>,
    },
    ReadChunkOk {
        path: String,
        start_line: u32,
        end_line: u32,
        bytes: usize,
    },
    WriteOk {
        path: String,
        bytes: usize,
    },
    WriteNoop {
        path: String,
        reason: String,
    },
    CmdOk {
        command: String,
        exit_code: i32,
        stdout_preview: String,   // first ~2 KB, already truncated
    },
    CmdFailed {
        command: String,
        exit_code: i32,
        stderr_preview: String,
    },

    // ── Cortex / Critic control events ────────────────────────────────
    CriticVeto {
        reason: String,
        /// True when the veto text mentioned line numbers / citations / grep
        mentions_citations: bool,
        /// Path of the artifact that was rejected (if any)
        artifact: Option<String>,
    },
    HaltRejected {
        kind: HaltRejectKind,     // Report | Symbols | Mission | …
        detail: String,
    },
    StrategyViolation {
        strategy: String,
        proposed_action: String,
        belief_before: f32,
        belief_after: f32,
    },
    CitationPreflight {
        command: String,
    },
    ParseFault {
        attempted_type: Option<String>,
        raw_preview: String,
    },
    PhaseTransition {
        from: String,
        to: String,
        reason: String,
    },
    NodePhase {
        message: String,
    },

    // ── Book-keeping ─────────────────────────────────────────────────
    CoverageUpdate {
        path: String,
        coverage_pct: f32,
        largest_gap: Option<(u32, u32)>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum HaltRejectKind {
    Report,
    Symbols,
    Mission,
    Other,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EvidenceLedger {
    events: Vec<EvidenceEvent>,
    /// Fast path for the most common queries
    last_write: HashMap<String, usize>,          // path → index of last WriteOk
    last_critic_veto: Option<usize>,
    last_halt_reject: Option<usize>,
}
