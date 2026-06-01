use crate::orchestrator_v2::task_graph::TaskNode;
use anyhow::Result;

pub struct ExecutorV2;

impl ExecutorV2 {
    /// Pure mechanical execution layer. No planning logic.
    pub async fn execute_node(_node: &TaskNode) -> Result<String> {
        // TODO: Route `assigned_agent` to physical capabilities (shell, AST edit).
        Ok("Execution payload output stub".into())
    }
}
