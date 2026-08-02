use super::proposal::*;

pub struct BuildExecutor;

impl BuildExecutor {
    pub fn validate(proposal: &BuildProposal) -> bool {
        proposal.requires_approval && !proposal.command.contains("rm")
    }
}
