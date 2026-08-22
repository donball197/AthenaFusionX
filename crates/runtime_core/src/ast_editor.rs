pub struct AstEditor;

impl AstEditor {
    /// STUB: real AST-aware function patching is not implemented.
    /// Fails loudly rather than silently corrupting the target file.
    pub fn update_function(
        _path: &str,
        _function_name: &str,
        _new_body: &str,
    ) -> anyhow::Result<String> {
        anyhow::bail!(
            "AstEditor::update_function is a stub — PatchFunction motor action is not yet implemented"
        )
    }
}
