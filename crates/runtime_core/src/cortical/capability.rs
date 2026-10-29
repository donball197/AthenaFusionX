//! Capability classification for agent actions.

use crate::agent_protocol::AgentAction;
use crate::policy::Capability;

/// Capability required to cross the ActionGate for an action.
///
/// This mapping must agree with the mutation classification used by the
/// READ-ONLY enforcement block in `handle_execute`: any variant refused
/// there as a mutation (`ReplaceLines`, `AtomicPatch` included) must
/// route to `WriteSubstrate` here. Historically those two variants fell
/// through to `_ => ReadSubstrate`, authorizing a write-type action as a
/// read at the gate.
pub(crate) fn capability_for(action: &AgentAction) -> Capability {
    match action {
        AgentAction::ExecuteCommand { .. } => Capability::ExecuteSystemCall,
        AgentAction::WriteFile { .. }
        | AgentAction::DiffPatch { .. }
        | AgentAction::PatchFunction { .. }
        | AgentAction::SynthesizeReport { .. }
        | AgentAction::ReplaceLines { .. }
        | AgentAction::AtomicPatch { .. } => Capability::WriteSubstrate,
        AgentAction::Halt { .. } => Capability::HaltCognition,
        // Read-class actions.
        AgentAction::ReadFile { .. }
        | AgentAction::ReadFileChunk { .. }
        | AgentAction::ReadFunction { .. }
        | AgentAction::SearchCode { .. }
        | AgentAction::SearchMemory { .. }
        | AgentAction::ScrapeUrl { .. } => Capability::ReadSubstrate,
        // UNREVIEWED: these fell through the old `_ => ReadSubstrate` wildcard.
        // Behavior preserved here; each needs a deliberate classification.
        // No wildcard on purpose: a new AgentAction variant must fail to
        // compile until it is classified.
        AgentAction::PatchFile { .. }
        | AgentAction::RunTests { .. }
        | AgentAction::CalculateEntropy { .. }
        | AgentAction::DelegateTask { .. }
        | AgentAction::ExportReport { .. }
        | AgentAction::FetchUiSnapshot { .. }
        | AgentAction::GitRollback { .. }
        | AgentAction::LazarusSnapshot { .. }
        | AgentAction::SpawnSubagent { .. }
        | AgentAction::VmoCreate { .. } => Capability::ReadSubstrate,
    }
}

/// Single source of truth for "does this action mutate the substrate".
/// Derived from `capability_for` so the ActionGate capability and the
/// READ-ONLY enforcement block in `handle_execute` cannot disagree.
pub(crate) fn is_mutation(action: &AgentAction) -> bool {
    matches!(capability_for(action), Capability::WriteSubstrate)
}
