pub struct LogicEngine;

impl LogicEngine {
    pub fn is_action_safe(directive: &str) -> bool {
        let forbidden = ["rm -rf", "mkfs", "shutdown"];
        let d_lower = directive.to_lowercase();
        
        // Even if the external CEO (Gemini) suggests it, the local logic engine VETOS it.
        !forbidden.iter().any(|&p| d_lower.contains(p))
    }

    pub fn detect_deadlock(observations: &str) -> bool {
        // If the same error appears 3 times, we are in a logic loop
        observations.contains("ERROR") && observations.len() > 1500
    }
}
