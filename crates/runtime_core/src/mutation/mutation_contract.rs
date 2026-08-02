#[derive(Debug, Clone)]
pub struct MutationContract {
    pub target_changed: bool,

    pub unrelated_files_unchanged: bool,

    pub syntax_valid: bool,

    pub tests_passed: bool,

    pub memory_recorded: bool,
}

impl MutationContract {
    pub fn verified(&self) -> bool {
        self.target_changed
            && self.unrelated_files_unchanged
            && self.syntax_valid
            && self.tests_passed
            && self.memory_recorded
    }
}
