use std::collections::HashMap;

pub struct SyntaxSentinel;

impl SyntaxSentinel {
    pub fn validate_safety(path: &str, content: &str) -> Result<(), String> {
        // Only perform deep syntax checks on Rust files for now
        if path.ends_with(".rs") {
            Self::check_rust_traps(content)?;
        }
        Ok(())
    }

    fn check_rust_traps(content: &str) -> Result<(), String> {
        let mut counts = HashMap::new();
        let mut in_string = false;
        let mut escaped = false;

        for (_i, c) in content.chars().enumerate() {
            if c == '\\' && !escaped {
                escaped = true;
                continue;
            }

            if c == '"' && !escaped {
                in_string = !in_string;
            }

            if !in_string {
                match c {
                    '{' => *counts.entry('{').or_insert(0) += 1,
                    '}' => *counts.entry('}').or_insert(0) += 1,
                    '(' => *counts.entry('(').or_insert(0) += 1,
                    ')' => *counts.entry(')').or_insert(0) += 1,
                    _ => {}
                }
            }
            escaped = false;
        }

        if in_string {
            return Err("FATAL: Unterminated string detected. Nexus tried to leave a quote open.".into());
        }

        if counts.get(&'{') != counts.get(&'}') {
            return Err("FATAL: Unbalanced curly braces. The logic tree is broken.".into());
        }

        if counts.get(&'(') != counts.get(&')') {
            return Err("FATAL: Unbalanced parentheses.".into());
        }

        Ok(())
    }
}
