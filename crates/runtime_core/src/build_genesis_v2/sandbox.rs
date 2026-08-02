pub struct SandboxBuilder;

impl SandboxBuilder {
    pub fn allowed(path: &str) -> bool {
        path.starts_with(".build_sandbox")
    }
}
