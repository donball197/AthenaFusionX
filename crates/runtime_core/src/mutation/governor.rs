#[derive(Debug)]
pub enum ApprovalDecision {
    Approved(String),
    Rejected(String),
}

pub struct MutationGovernor;

impl MutationGovernor {
    pub fn evaluate(
        files_changed: usize,
        tests_passed: bool,
        backup_exists: bool,
    ) -> ApprovalDecision {
        if files_changed > 1 {
            return ApprovalDecision::Rejected("mutation scope exceeds policy".into());
        }

        if !tests_passed {
            return ApprovalDecision::Rejected("verification failed".into());
        }

        if !backup_exists {
            return ApprovalDecision::Rejected("rollback unavailable".into());
        }

        ApprovalDecision::Approved("mutation satisfies safety policy".into())
    }
}
