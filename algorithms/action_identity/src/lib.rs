use std::hash::{Hash, Hasher};

/// Runtime-neutral representation of the fields that define an AgentAction's
/// canonical anti-loop identity.
///
/// This type intentionally does not depend on runtime_core::AgentAction.
/// Runtime-specific conversion remains at the Cortex boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionIdentityInput<'a> {
    ReadFile { path: &'a str },
    ReadFunction { path: &'a str, function_name: &'a str },
    ReadFileChunk { path: &'a str, start_line: usize },
    SearchCode { path: &'a str, query: &'a str },
    SearchMemory { query: &'a str },
    WriteFile { path: &'a str, content: &'a str },
    DiffPatch { path: &'a str },
    PatchFunction { path: &'a str, function_name: &'a str, new_body: &'a str },
    SynthesizeReport { output_path: &'a str },
    ExecuteCommand { command: &'a str },
    Halt,
}

/// Calculate the canonical anti-loop identity for a normalized action.
///
/// This preserves the existing Cortex behavior exactly:
/// - reasoning/confidence are excluded;
/// - WriteFile identity includes content;
/// - PatchFunction identity includes new_body;
/// - ExecuteCommand trims surrounding whitespace;
/// - Halt is identified solely by action type.
pub fn action_key(input: ActionIdentityInput<'_>) -> String {
    match input {
        ActionIdentityInput::ReadFile { path } => {
            format!("read_file:{}", path)
        }
        ActionIdentityInput::ReadFunction { path, function_name } => {
            format!("read_function:{}:{}", path, function_name)
        }
        ActionIdentityInput::ReadFileChunk { path, start_line } => {
            format!("read_file_chunk:{}:{}", path, start_line)
        }
        ActionIdentityInput::SearchCode { path, query } => {
            format!("search_code:{}:{}", path, query)
        }
        ActionIdentityInput::SearchMemory { query } => {
            format!("search_memory:{}", query)
        }
        ActionIdentityInput::WriteFile { path, content } => {
            format!("write_file:{}:{:016x}", path, hash_value(content))
        }
        ActionIdentityInput::DiffPatch { path } => {
            format!("diff_patch:{}", path)
        }
        ActionIdentityInput::PatchFunction { path, function_name, new_body } => {
            format!("patch_function:{}:{}:{:016x}", path, function_name, hash_value(new_body))
        }
        ActionIdentityInput::SynthesizeReport { output_path } => {
            format!("synthesize_report:{}", output_path)
        }
        ActionIdentityInput::ExecuteCommand { command } => {
            format!("execute_command:{}", command.trim())
        }
        ActionIdentityInput::Halt => "halt".to_string(),
    }
}

fn hash_value(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_file_identity_is_path_based() {
        let a = action_key(ActionIdentityInput::ReadFile { path: "a.rs" });
        let b = action_key(ActionIdentityInput::ReadFile { path: "a.rs" });
        let c = action_key(ActionIdentityInput::ReadFile { path: "b.rs" });

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, "read_file:a.rs");
    }

    #[test]
    fn read_function_identity_contains_path_and_function() {
        assert_eq!(
            action_key(ActionIdentityInput::ReadFunction {
                path: "src/lib.rs",
                function_name: "foo",
            }),
            "read_function:src/lib.rs:foo"
        );
    }

    #[test]
    fn read_file_chunk_identity_contains_start_line() {
        assert_eq!(
            action_key(ActionIdentityInput::ReadFileChunk { path: "src/lib.rs", start_line: 42 }),
            "read_file_chunk:src/lib.rs:42"
        );
    }

    #[test]
    fn search_code_identity_contains_path_and_query() {
        assert_eq!(
            action_key(ActionIdentityInput::SearchCode { path: "src", query: "Cargo.toml" }),
            "search_code:src:Cargo.toml"
        );
    }

    #[test]
    fn search_memory_identity_contains_query() {
        assert_eq!(
            action_key(ActionIdentityInput::SearchMemory { query: "math_utils" }),
            "search_memory:math_utils"
        );
    }

    #[test]
    fn write_file_identity_is_content_sensitive() {
        let a = action_key(ActionIdentityInput::WriteFile { path: "Cargo.toml", content: "one" });
        let b = action_key(ActionIdentityInput::WriteFile { path: "Cargo.toml", content: "one" });
        let c = action_key(ActionIdentityInput::WriteFile { path: "Cargo.toml", content: "two" });

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("write_file:Cargo.toml:"));
    }

    #[test]
    fn patch_function_identity_is_body_sensitive() {
        let base = action_key(ActionIdentityInput::PatchFunction {
            path: "apps/math_utils/src/lib.rs",
            function_name: "add",
            new_body: "a + b",
        });
        let same = action_key(ActionIdentityInput::PatchFunction {
            path: "apps/math_utils/src/lib.rs",
            function_name: "add",
            new_body: "a + b",
        });
        let diff = action_key(ActionIdentityInput::PatchFunction {
            path: "apps/math_utils/src/lib.rs",
            function_name: "add",
            new_body: "a - b",
        });

        assert_eq!(base, same);
        assert_ne!(base, diff);
        assert!(base.starts_with("patch_function:apps/math_utils/src/lib.rs:add:"));
    }

    #[test]
    fn execute_command_trims_whitespace() {
        assert_eq!(
            action_key(ActionIdentityInput::ExecuteCommand { command: "  cargo test  " }),
            "execute_command:cargo test"
        );
    }

    #[test]
    fn halt_identity_ignores_reasoning_by_design() {
        assert_eq!(action_key(ActionIdentityInput::Halt), "halt");
    }

    #[test]
    fn diff_patch_identity_is_path_based() {
        assert_eq!(
            action_key(ActionIdentityInput::DiffPatch { path: "src/lib.rs" }),
            "diff_patch:src/lib.rs"
        );
    }

    #[test]
    fn synthesize_report_identity_is_output_path_based() {
        assert_eq!(
            action_key(ActionIdentityInput::SynthesizeReport { output_path: "report.md" }),
            "synthesize_report:report.md"
        );
    }
}
