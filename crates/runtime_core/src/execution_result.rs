//! Structured execution results — replaces raw String returns from RoboticExecutor.
//! Additive module: does not break existing code. Wire incrementally.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Success { marker: EvidenceMarker, output: String },
    CompilerFault { stderr: String, auto_healed: bool },
    Reverted { reason: String, critique: String },
    ActionFailed { error: String },
    Blocked { reason: String },
    Halted { reasoning: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceMarker {
    ReadOk, ReadChunkOk, SearchCodeOk, SearchOk, CacheHit, CmdOk,
    WriteOk, WriteNoop, DiffPatchOk, PatchFunctionOk, ReplaceLinesOk,
    VerifyOk, CompileOk, CompileFail, AutoHeal, MutationReverted,
    ActionFailed, ActionGateBlocked, Halt,
}

impl EvidenceMarker {
    pub fn as_bracketed(&self) -> &'static str {
        match self {
            Self::ReadOk => "[READ_OK]", Self::ReadChunkOk => "[READ_CHUNK_OK]",
            Self::SearchCodeOk => "[SEARCH_CODE_OK]", Self::SearchOk => "[SEARCH_OK]",
            Self::CacheHit => "[CACHE_HIT]", Self::CmdOk => "[CMD_OK]",
            Self::WriteOk => "[WRITE_OK]", Self::WriteNoop => "[WRITE_NOOP]",
            Self::DiffPatchOk => "[DIFF_PATCH_OK]", Self::PatchFunctionOk => "[PATCH_FUNCTION_OK]",
            Self::ReplaceLinesOk => "[REPLACE_LINES_OK]", Self::VerifyOk => "[VERIFY_OK]",
            Self::CompileOk => "[COMPILE_OK]", Self::CompileFail => "[COMPILE_FAIL]",
            Self::AutoHeal => "[AUTO_HEAL]", Self::MutationReverted => "[MUTATION_REVERTED]",
            Self::ActionFailed => "[ACTION_FAILED]", Self::ActionGateBlocked => "[ACTION_GATE_BLOCKED]",
            Self::Halt => "[HALT]",
        }
    }
    
    pub fn is_mutation_success(&self) -> bool {
        matches!(self, Self::WriteOk | Self::DiffPatchOk | Self::PatchFunctionOk | Self::ReplaceLinesOk)
    }
    pub fn is_mutation_failure(&self) -> bool {
        matches!(self, Self::MutationReverted | Self::CompileFail | Self::ActionFailed)
    }
    pub fn is_recon(&self) -> bool {
        matches!(self, Self::ReadOk | Self::ReadChunkOk | Self::SearchCodeOk | Self::SearchOk | Self::CacheHit | Self::CmdOk)
    }
}

impl ExecutionResult {
    pub fn as_evidence_text(&self) -> String {
        match self {
            Self::Success { marker, output } => {
                if output.trim().is_empty() { marker.as_bracketed().to_string() }
                else { format!("{} {}", marker.as_bracketed(), output) }
            }
            Self::CompilerFault { stderr, auto_healed } => {
                if *auto_healed { "[AUTO_HEAL] Compiler error auto-fixed. Re-checking...".to_string() }
                else { format!("[COMPILE_FAIL] Mutation caused compilation error:\n{}", stderr) }
            }
            Self::Reverted { reason, critique } => format!("[MUTATION_REVERTED] {} -- Evaluator rejected: {}", reason, critique),
            Self::ActionFailed { error } => format!("[ACTION_FAILED] OS rejected action: {}", error),
            Self::Blocked { reason } => format!("[ACTION_GATE_BLOCKED] {}", reason),
            Self::Halted { reasoning } => format!("[HALT] {}", reasoning),
        }
    }
    
    pub fn marker(&self) -> Option<EvidenceMarker> {
        match self {
            Self::Success { marker, .. } => Some(*marker),
            Self::CompilerFault { auto_healed, .. } => if *auto_healed { Some(EvidenceMarker::AutoHeal) } else { Some(EvidenceMarker::CompileFail) },
            Self::Reverted { .. } => Some(EvidenceMarker::MutationReverted),
            Self::ActionFailed { .. } => Some(EvidenceMarker::ActionFailed),
            Self::Blocked { .. } => Some(EvidenceMarker::ActionGateBlocked),
            Self::Halted { .. } => Some(EvidenceMarker::Halt),
        }
    }
    
    pub fn is_mutation_success(&self) -> bool { self.marker().map(|m| m.is_mutation_success()).unwrap_or(false) }
    pub fn is_mutation_failure(&self) -> bool { self.marker().map(|m| m.is_mutation_failure()).unwrap_or(false) }
    pub fn is_recon(&self) -> bool { self.marker().map(|m| m.is_recon()).unwrap_or(false) }
}
