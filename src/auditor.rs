pub struct SyntaxSentinel;

impl SyntaxSentinel {
    pub fn validate_safety(path: &str, content: &str) -> Result<(), String> {
        // 1. Block destructive shell commands in Rust code
        if content.contains("std::process::Command") && content.contains("rm -rf") {
            return Err("Hypothesis Blocked: Destructive Command Detected.".into());
        }
        
        // 2. Block infinite loop risks in build scripts
        if content.contains("loop {") && !content.contains("sleep") && !content.contains("break") {
            return Err("Hypothesis Blocked: Potential Infinite Loop without Yield.".into());
        }

        // 3. ARCHITECTURAL GUARD: Prevent "Nesting Sabotage"
        // Blocks the AI from wrapping everything in a 'mod nexus_core' block in main.rs
        if path.ends_with("main.rs") && content.contains("mod nexus_core {") {
            return Err("Architectural Sabotage Detected: Module nesting 'nexus_core' is forbidden. Maintain flat hierarchy.".into());
        }

        Ok(())
    }
}
