//! Runtime adapter for the independent action-identity algorithm.
//!
//! AgentAction remains owned by runtime_core. The extracted algorithm
//! receives only the fields that define canonical identity. Variants that
//! historically used the Debug fallback preserve that exact behavior.

use crate::agent_protocol::AgentAction;
use nexus_action_identity_algorithms::{action_key as calculate_action_key, ActionIdentityInput};

/// Canonical anti-loop key for an action.
///
/// Two actions with the same key are treated as duplicates. For
/// `PatchFunction`, the key includes a hash of `new_body` so a
/// corrective retry on the same symbol with a different body is not
/// filtered as a repeat of an already-executed attempt.
///
/// `reasoning` is excluded from identity for every variant except
/// `Halt`, whose answer text IS its content: a revised halt after a
/// rejection must not be filtered as a duplicate, so halt identity is
/// a hash of its full Debug text (which includes `reasoning`). This
/// exception is deliberate.
pub(crate) fn action_key(action: &AgentAction) -> String {
    let input = match action {
        AgentAction::ReadFile { path, .. } => Some(ActionIdentityInput::ReadFile { path }),
        AgentAction::ReadFunction { path, function_name, .. } => {
            Some(ActionIdentityInput::ReadFunction { path, function_name })
        }
        AgentAction::ReadFileChunk { path, start_line, .. } => {
            Some(ActionIdentityInput::ReadFileChunk { path, start_line: *start_line })
        }
        AgentAction::SearchCode { path, query, .. } => {
            Some(ActionIdentityInput::SearchCode { path, query })
        }
        AgentAction::SearchMemory { query, .. } => {
            Some(ActionIdentityInput::SearchMemory { query })
        }
        AgentAction::WriteFile { path, content, .. } => {
            Some(ActionIdentityInput::WriteFile { path, content })
        }
        AgentAction::DiffPatch { path, .. } => Some(ActionIdentityInput::DiffPatch { path }),
        AgentAction::PatchFunction { path, function_name, new_body, .. } => {
            Some(ActionIdentityInput::PatchFunction { path, function_name, new_body })
        }
        AgentAction::SynthesizeReport { output_path, .. } => {
            Some(ActionIdentityInput::SynthesizeReport { output_path })
        }
        AgentAction::ExecuteCommand { command, .. } => {
            Some(ActionIdentityInput::ExecuteCommand { command })
        }
        _ => None,
    };

    // A halt carries its answer in `reasoning`. Key on a hash of the
    // text so a revised halt after a rejection is not a "duplicate".
    if let AgentAction::Halt { .. } = action {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", action).hash(&mut h);
        return format!("halt:{:016x}", h.finish());
    }

    match input {
        Some(input) => calculate_action_key(input),
        None => format!("{:?}", action),
    }
}

/// FIX BUG-17: canonical "does this action touch `path`" predicate, used by
/// the code-shift listener to distinguish self-inflicted mutations from
/// external ones. Prefer this over substring matching on `local_evidence`,
/// which also matches inherited DAG lineage and critic text.
pub(crate) fn action_touches_path(action: &crate::agent_protocol::AgentAction, path: &str) -> bool {
    use crate::agent_protocol::AgentAction as A;
    match action {
        A::PatchFile { path: p, .. }
        | A::ReadFunction { path: p, .. }
        | A::ReadFile { path: p, .. }
        | A::ReadFileChunk { path: p, .. }
        | A::SearchCode { path: p, .. }
        | A::WriteFile { path: p, .. }
        | A::ReplaceLines { path: p, .. }
        | A::DiffPatch { path: p, .. }
        | A::SynthesizeReport { output_path: p, .. }
        | A::PatchFunction { path: p, .. } => p == path,
        A::AtomicPatch { patches, .. } => patches.iter().any(|fp| fp.path == path),
        _ => false,
    }
}
