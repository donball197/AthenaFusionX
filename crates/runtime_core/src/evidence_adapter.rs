//! Adapter bridging runtime_core executor evidence with typed
//! `nexus_evidence_requirements::evidence::Evidence` and `evidence_markers.rs`
//! reliability recomputation.

use crate::agent_protocol::AgentAction;
use crate::evidence_markers::{has_patch_satisfied_marker, has_recon_marker, write_path_satisfied};
use crate::evidence_paths::{cmd_ok_binds_command, normalize_path, read_path_satisfied};
use nexus_evidence_requirements::assessment::Assessment;
use nexus_evidence_requirements::evidence::{
    ActionIdentity, Evidence as TypedEvidence, Scope, TargetId,
};
use nexus_evidence_requirements::predicate::Predicate;
use nexus_evidence_requirements::predicates::ReadPathEvidenceEstablished;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypedEvidenceKind {
    Read,
    ReadFunction,
    Search,
    SearchMemory,
    Write,
    Patch,
    Command,
    Halt,
    Failure,
    Other,
}

#[derive(Clone, Debug)]
pub struct TypedEvidenceRecord {
    pub evidence: TypedEvidence,
    pub kind: TypedEvidenceKind,
    /// Deterministic reliability in [0.0, 1.0], recomputed from
    /// `evidence_markers.rs` / `evidence_paths.rs` rather than `ProvenanceLevel`.
    pub reliability: f32,
}

impl TypedEvidenceRecord {
    pub fn render_ledger_line(&self) -> String {
        let target = self.evidence.target.as_ref().map(|t| t.0.as_str()).unwrap_or("-");
        let scope = self.evidence.scope.as_ref().map(|s| s.0.as_str()).unwrap_or("-");
        format!(
            "- kind={:?} scope={} target={} success={} reliability={:.2}",
            self.kind, scope, target, self.evidence.success, self.reliability
        )
    }
}

fn canonical_target(p: &str) -> String {
    let n = normalize_path(p);
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_s = cwd.to_string_lossy();
        let prefix = format!("{}/", cwd_s.trim_end_matches('/'));
        if let Some(rel) = n.strip_prefix(&prefix) {
            return normalize_path(rel);
        }
    }
    n
}

/// Recompute physical reliability directly from deterministic markers in
/// `evidence_markers.rs` / `evidence_paths.rs`.
pub fn recompute_reliability(raw: &str) -> f32 {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    if trimmed.contains("[ACTION FAILED]") || trimmed.contains("PATH_TRAVERSAL_DENIED") {
        if crate::evidence_paths::is_confirmed_absence(trimmed) {
            return 0.70;
        }
        return 0.0;
    }
    if trimmed.contains("[WRITE_OK]")
        || has_patch_satisfied_marker(trimmed)
        || cmd_ok_binds_command(trimmed)
        || trimmed.contains("[READ_OK]")
        || trimmed.contains("[READ_CHUNK_OK]")
        || trimmed.contains("[READ_FUNCTION_OK]")
    {
        return 1.0;
    }
    if trimmed.contains("[WRITE_NOOP]")
        || trimmed.contains("[SEARCH_CODE_OK]")
        || trimmed.contains("[SEARCH_CHECKED_OK]")
        || trimmed.contains("[SEARCH_MEMORY_OK]")
        || has_recon_marker(trimmed)
    {
        return 0.85;
    }
    if trimmed.contains("[HALT]") {
        return 0.50;
    }
    0.10
}

pub fn from_executed_action(action: &AgentAction, raw_evidence: &str) -> TypedEvidenceRecord {
    let action_id = ActionIdentity(crate::cortical::action_identity::action_key(action));
    let (kind, scope_str, target_str, success) = match action {
        AgentAction::ReadFile { path, .. } | AgentAction::ReadFileChunk { path, .. } => {
            let p = canonical_target(path);
            let ok = read_path_satisfied(raw_evidence, &p);
            (TypedEvidenceKind::Read, "read", Some(p), ok)
        }
        AgentAction::ReadFunction { path, function_name, .. } => {
            let p = canonical_target(path);
            let ok = crate::evidence_markers::read_function_marker_satisfies(
                raw_evidence,
                &p,
                function_name,
            );
            (
                TypedEvidenceKind::ReadFunction,
                "read_function",
                Some(format!("{}:{}", p, function_name.trim())),
                ok,
            )
        }
        AgentAction::SearchCode { path, query, .. } => {
            let ok = raw_evidence.contains("[SEARCH_CODE_OK]")
                || raw_evidence.contains("[SEARCH_CHECKED_OK]");
            let target = format!("{}:{}", canonical_target(path), query.trim());
            (TypedEvidenceKind::Search, "search_code", Some(target), ok)
        }
        AgentAction::SearchMemory { query, .. } => {
            let ok = raw_evidence.contains("[SEARCH_MEMORY_OK]");
            (TypedEvidenceKind::SearchMemory, "search_memory", Some(query.trim().to_string()), ok)
        }
        AgentAction::WriteFile { path, .. } => {
            let p = canonical_target(path);
            let ok = write_path_satisfied(raw_evidence, &p);
            (TypedEvidenceKind::Write, "write", Some(p), ok)
        }
        AgentAction::SynthesizeReport { output_path, .. } => {
            let p = canonical_target(output_path);
            let ok = write_path_satisfied(raw_evidence, &p);
            (TypedEvidenceKind::Write, "write", Some(p), ok)
        }
        AgentAction::PatchFunction { path, .. }
        | AgentAction::PatchFile { path, .. }
        | AgentAction::DiffPatch { path, .. }
        | AgentAction::ReplaceLines { path, .. } => {
            let p = canonical_target(path);
            let ok = has_patch_satisfied_marker(raw_evidence);
            (TypedEvidenceKind::Patch, "patch", Some(p), ok)
        }
        AgentAction::ExecuteCommand { command, .. } => {
            let norm = crate::cmd_match::normalize_cmd(command);
            let ok = cmd_ok_binds_command(raw_evidence);
            (TypedEvidenceKind::Command, "command", Some(norm), ok)
        }
        AgentAction::Halt { .. } => {
            let ok = raw_evidence.contains("[HALT]");
            (TypedEvidenceKind::Halt, "halt", None, ok)
        }
        _ => (TypedEvidenceKind::Other, "other", None, false),
    };

    let reliability = if success {
        recompute_reliability(raw_evidence).max(0.70)
    } else {
        recompute_reliability(raw_evidence).min(0.25)
    };

    TypedEvidenceRecord {
        evidence: TypedEvidence::new(
            action_id,
            target_str.map(TargetId),
            Some(Scope(scope_str.to_string())),
            success,
            Some(raw_evidence.chars().take(512).collect()),
        ),
        kind,
        reliability,
    }
}

/// Parse raw evidence lines into typed `nexus_evidence_requirements::evidence::Evidence`
/// records for verifier consumption.
pub fn parse_read_evidence_records(evidence: &str) -> Vec<TypedEvidence> {
    let mut out = Vec::new();
    for line in evidence.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("[READ_OK]") {
            let path = canonical_target(rest.trim());
            if !path.is_empty() {
                out.push(TypedEvidence::new(
                    ActionIdentity(format!("read_file:{}", path)),
                    Some(TargetId(path)),
                    Some(Scope("read".to_string())),
                    true,
                    Some(line.to_string()),
                ));
            }
        } else if let Some(rest) = t.strip_prefix("[READ_CHUNK_OK] File:") {
            if let Some(path_raw) = rest.split(" | ").next() {
                let path = canonical_target(path_raw.trim());
                if !path.is_empty() {
                    out.push(TypedEvidence::new(
                        ActionIdentity(format!("read_file_chunk:{}", path)),
                        Some(TargetId(path)),
                        Some(Scope("read".to_string())),
                        true,
                        Some(line.to_string()),
                    ));
                }
            }
        } else if let Some(rest) = t.strip_prefix("[SEARCH_CODE_OK] Path:") {
            if let Some(path_raw) = rest.split(" | ").next() {
                let path = canonical_target(path_raw.trim());
                if !path.is_empty() {
                    out.push(TypedEvidence::new(
                        ActionIdentity(format!("search_code:{}", path)),
                        Some(TargetId(path)),
                        Some(Scope("read".to_string())),
                        true,
                        Some(line.to_string()),
                    ));
                }
            }
        } else if let Some(rest) = t.strip_prefix("[ACTION FAILED] path=") {
            if let Some(path_raw) = rest.split(" OS rejected action").next() {
                let path = canonical_target(path_raw.trim());
                let confirmed = crate::evidence_paths::is_confirmed_absence(rest);
                if !path.is_empty() {
                    out.push(TypedEvidence::new(
                        ActionIdentity(format!("read_file:{}", path)),
                        Some(TargetId(path)),
                        Some(Scope("read".to_string())),
                        confirmed,
                        Some(line.to_string()),
                    ));
                }
            }
        }
    }
    out
}

/// Evaluate a typed `ReadPathEvidenceEstablished` predicate across parsed records.
pub fn assess_read_path_typed(evidence: &str, expected_path: &str) -> Assessment {
    let pred = ReadPathEvidenceEstablished {
        target: TargetId(canonical_target(expected_path)),
        scope: Scope("read".to_string()),
    };
    let records = parse_read_evidence_records(evidence);
    records
        .iter()
        .map(|ev| pred.assess(Some(ev)))
        .filter(|a| *a != Assessment::NN)
        .reduce(Assessment::any)
        .unwrap_or(Assessment::NN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recomputes_reliability_for_all_real_markers() {
        assert_eq!(recompute_reliability("[WRITE_OK] a.md"), 1.0);
        assert_eq!(recompute_reliability("[PATCH_FUNCTION_OK] a.rs:f"), 1.0);
        assert_eq!(recompute_reliability("[READ_CHUNK_OK] File: a.rs | Lines 1-10 of 20"), 1.0);
        assert!(recompute_reliability("[SEARCH_CODE_OK] Path: src | Query: 'x'") >= 0.80);
        assert_eq!(recompute_reliability("[ACTION FAILED] OS rejected action: denied"), 0.0);
    }

    #[test]
    fn typed_read_predicate_assesses_sc_rc_and_nn() {
        let sc = assess_read_path_typed(
            "[READ_CHUNK_OK] File: ./src/lib.rs | Lines 1-10 of 10",
            "src/lib.rs",
        );
        assert_eq!(sc, Assessment::SC);

        let rc = assess_read_path_typed(
            "[ACTION FAILED] path=src/lib.rs OS rejected action: denied",
            "src/lib.rs",
        );
        assert_eq!(rc, Assessment::RC);

        let nn = assess_read_path_typed("[READ_OK] src/other.rs", "src/lib.rs");
        assert_eq!(nn, Assessment::NN);
    }
}
