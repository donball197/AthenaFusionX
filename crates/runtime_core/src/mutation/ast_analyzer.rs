#[derive(Debug, Clone)]
pub struct AstMutationCandidate {
    pub original: String,

    pub replacement: String,

    pub reason: String,
}

pub struct AstAnalyzer;

impl AstAnalyzer {
    pub fn analyze(original: &str, replacement: &str) -> AstMutationCandidate {
        AstMutationCandidate {
            original: original.to_string(),

            replacement: replacement.to_string(),

            reason: "candidate mutation generated".to_string(),
        }
    }
}
