// crates/runtime_core/src/cortical/terminal_gate.rs

use crate::cognition::epistemic_session::EpistemicSession;

/// Authoritative evaluator for terminal completion conditions across cognitive states.
#[allow(dead_code)]
pub struct TerminalGate {
    pub contract_satisfied: bool,
    pub compile_remediation_resolved: bool,
    pub is_command_only: bool,
    pub requires_semantic_verification: bool,
}

impl TerminalGate {
    pub fn evaluate(session: &EpistemicSession, belief: f32) -> Self {
        let contract_satisfied = crate::completion_engine::SubgoalCompletionEngine::evaluate(
            &session.contract,
            &session.local_evidence,
            belief,
        );
        let compile_remediation_resolved = !session.compile_remediation.is_active();
        let is_command_only = session.contract.conditions.len() == 1
            && matches!(
                session.contract.conditions.first(),
                Some(crate::completion_engine::CompletionCondition::CommandExecutionVerified { .. })
            );
        let requires_semantic_verification =
            crate::completion_engine::requires_semantic_verification(&session.contract.objective);

        Self {
            contract_satisfied,
            compile_remediation_resolved,
            is_command_only,
            requires_semantic_verification,
        }
    }

    pub fn is_autonomous_completion_satisfied(&self) -> bool {
        self.contract_satisfied
            && self.compile_remediation_resolved
            && !self.requires_semantic_verification
    }

    #[allow(dead_code)] // sibling of is_autonomous_completion_satisfied without the remediation gate; intentionally unwired
    pub fn is_pure_command_satisfied(&self) -> bool {
        self.contract_satisfied && self.is_command_only && !self.requires_semantic_verification
    }
}
