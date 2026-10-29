use anyhow::{Context, Result};
use nexus_ast_tool::mutation::{MutationStrategy, ReplaceFunctionBody};
use std::fs;

pub struct AstEditor;

impl AstEditor {
    /// AST-aware function patching powered by nexus_ast_tool's ReplaceFunctionBody.
    /// Safely replaces the body of the target function or impl method while
    /// preserving external comments, visibility, attributes, and file formatting.
    pub fn update_function(path: &str, function_name: &str, new_body: &str) -> Result<String> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("AstEditor failed to read source file at '{}'", path))?;

        let strategy = ReplaceFunctionBody;
        let patched = strategy.apply(&source, function_name, new_body).map_err(|e| {
            anyhow::anyhow!(
                "AstEditor failed to patch function '{}' in '{}': {}",
                function_name,
                path,
                e
            )
        })?;

        Ok(patched)
    }
}

#[cfg(test)]
mod tests {
    use super::AstEditor;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn patches_free_function_body() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "// Header comment
fn calculate(x: i32) -> i32 {{
    x * 2
}}
"
        )
        .unwrap();

        let patched =
            AstEditor::update_function(file.path().to_str().unwrap(), "calculate", "x * 10 + 5")
                .unwrap();

        assert!(patched.contains("// Header comment"));
        assert!(patched.contains("fn calculate(x: i32) -> i32"));
        assert!(patched.contains("x * 10 + 5"));
        assert!(!patched.contains("x * 2"));
    }

    #[test]
    fn patches_impl_method_body() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "struct Math;
impl Math {{
    pub fn evaluate(&self) -> bool {{
        false
    }}
}}
"
        )
        .unwrap();

        let patched =
            AstEditor::update_function(file.path().to_str().unwrap(), "evaluate", "true").unwrap();

        assert!(patched.contains("struct Math;"));
        assert!(patched.contains("pub fn evaluate(&self) -> bool"));
        assert!(patched.contains("true"));
        assert!(!patched.contains("false"));
    }

    #[test]
    fn accepts_full_fn_declaration_payload_gracefully() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "fn answer() -> i32 {{
    0
}}"
        )
        .unwrap();

        // Model sometimes outputs the full fn signature inside new_body
        let patched = AstEditor::update_function(
            file.path().to_str().unwrap(),
            "answer",
            "fn answer() -> i32 { 42 }",
        )
        .unwrap();

        assert!(patched.contains("42"));
        assert!(!patched.contains("0"));
    }
}
