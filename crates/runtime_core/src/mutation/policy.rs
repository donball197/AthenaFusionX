pub struct MutationPolicy {
    pub max_files_changed: usize,
    pub require_tests: bool,
    pub require_backup: bool,
}

impl Default for MutationPolicy {
    fn default() -> Self {
        Self { max_files_changed: 1, require_tests: true, require_backup: true }
    }
}
