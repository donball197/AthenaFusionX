use crate::agent_protocol::AgentAction;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PatchClass {
    Mutation { path: String, function_name: String },
    Idempotent { path: String, function_name: String },
    NotPatch,
}

pub(crate) async fn classify_patch_action(action: &AgentAction) -> anyhow::Result<PatchClass> {
    let AgentAction::PatchFunction { _thought_process: None, path, function_name, new_body, .. } = action else {
        return Ok(PatchClass::NotPatch);
    };

    let original = tokio::fs::read_to_string(path).await?;
    let patched = nexus_ast_tool::mutation::ReplaceFunctionBody::apply_for_path(
        std::path::Path::new(path),
        &original,
        function_name,
        new_body,
    )?;

    Ok(if patched != original {
        PatchClass::Mutation { path: path.clone(), function_name: function_name.clone() }
    } else {
        PatchClass::Idempotent { path: path.clone(), function_name: function_name.clone() }
    })
}
