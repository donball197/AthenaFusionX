#[derive(Debug, Clone)]
pub struct PatchFunction {
    pub target_function: String,

    pub old_hash: String,

    pub new_body: String,

    pub mutation_reason: String,
}

impl PatchFunction {
    pub fn describe(&self) -> String {
        format!("PatchFunction:{}", self.target_function)
    }
}
