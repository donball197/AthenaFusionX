pub struct BuildPolicy;

impl BuildPolicy {
    pub fn allowed(command: &str) -> bool {
        command.starts_with("cargo build")
    }
}
