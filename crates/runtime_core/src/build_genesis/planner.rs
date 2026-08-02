use super::proposal::*;

pub struct BuildPlanner;

impl BuildPlanner {
    pub fn create_workspace_build() -> BuildProposal {
        BuildProposal::controlled("workspace")
    }
}
