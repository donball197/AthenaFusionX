#[derive(Debug)]
pub enum MutationState {
    Created,

    BackupComplete,

    Patched,

    Verified,

    RolledBack,
}

pub struct MutationTransaction {
    pub state: MutationState,
}

impl MutationTransaction {
    pub fn begin() -> Self {
        Self { state: MutationState::Created }
    }
}
