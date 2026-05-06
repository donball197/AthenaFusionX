use anyhow::Result;
use crate::agent_protocol::AgentAction;
use crate::json_parser::extract_actions;

#[derive(Debug)]
pub struct RepairAction {
    pub action_type: String,
    pub path: String,
    pub search: Option<String>,
    pub replace: Option<String>,
    pub function_name: Option<String>,
    pub new_body: Option<String>,
}

pub fn to_repair_actions(raw: &str) -> Result<Vec<RepairAction>> {
    let actions = extract_actions(raw)?;
    let mut repairs = Vec::new();

    for action in actions {
        match action {
            AgentAction::DiffPatch { path, search, replace, .. } => {
                repairs.push(RepairAction {
                    action_type: "diff_patch".to_string(),
                    path,
                    search: Some(search),
                    replace: Some(replace),
                    function_name: None,
                    new_body: None,
                });
            }
            AgentAction::PatchFunction { path, function_name, new_body, .. } => {
                repairs.push(RepairAction {
                    action_type: "patch_function".to_string(),
                    path,
                    search: None,
                    replace: None,
                    function_name: Some(function_name),
                    new_body: Some(new_body),
                });
            }
            _ => {}
        }
    }
    Ok(repairs)
}
