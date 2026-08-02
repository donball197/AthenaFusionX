use crate::genome::Genome;

pub struct PromptCompiler;

impl PromptCompiler {
    pub fn compile_planner_prompt(genome: &Genome) -> String {
        format!(
            "SYSTEM_POLICY: [PLANNER]\nStyle: {:?}\nDecomposition: {}\nBranching: {}\nRisk Threshold: {:.2}\n\nSTRICT CONSTRAINTS: Follow the defined architectural patterns.",
            genome.genes.planning.style,
            genome.genes.planning.decomposition_depth,
            genome.genes.planning.branch_factor,
            genome.genes.risk_threshold
        )
    }

    pub fn compile_critic_prompt(genome: &Genome) -> String {
        format!(
            "SYSTEM_POLICY: [CRITIC]\nStyle: {:?}\nVeto Threshold: {:.2}\nEvidence Weight: {:.2}\n\nSTRICT CONSTRAINTS: Evaluate without bias according to strict metrics.",
            genome.genes.critic.style,
            genome.genes.critic.veto_threshold,
            genome.genes.critic.evidence_weight
        )
    }
}
