pub struct BuildGovernor;

impl BuildGovernor {
    pub fn approve(files_changed: usize) -> bool {
        files_changed <= 5
    }
}
