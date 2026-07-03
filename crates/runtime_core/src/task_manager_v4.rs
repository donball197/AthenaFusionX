use crate::cognition::meta_reasoner::MetaReasoner;
use crate::task_manager::TaskContext;
use anyhow::Result;

pub async fn send_task_v4(ctx: TaskContext, directive: &str) -> Result<String> {
    MetaReasoner::execute_with_got(&ctx.client, &ctx.api_key, directive).await.map_err(|e| anyhow::anyhow!(e))
}
