#!/bin/bash

# Refactor: Encapsulate arguments into TaskContext for Nexus v1.9
# Usage: ./apply_refactor.sh

# 1. Append TaskContext struct to task_manager.rs
cat << 'EOF' >> crates/runtime_core/src/task_manager.rs

pub struct TaskContext {
    pub orchestrator: Arc<Orchestrator>,
    pub client: Client,
    pub api_key: String,
    pub directive: String,
    pub memory: Arc<tokio::sync::Mutex<MemoryPalace>>,
    pub vmo: Arc<tokio::sync::Mutex<VmoState>>,
    pub tx: tokio::sync::broadcast::Sender<String>,
    pub dist: Arc<DistributedScheduler>,
}
EOF

# 2. Update send_task signature in task_manager.rs 
# Replaces 'pub async fn send_task(...)' with the struct-based definition
sed -i 's/pub async fn send_task(/pub async fn send_task(ctx: TaskContext) -> Result<Response, Error> { \/\/ /g' crates/runtime_core/src/task_manager.rs

# 3. Update main.rs call site
# We use a python3 one-liner to perform the multiline replacement accurately
python3 -c "
import sys
path = 'bins/cli/src/main.rs'
with open(path, 'r') as f:
    content = f.read()

# The specific block to replace
old_block = \"\"\"let response = send_task(
                     orchestrator.clone(),
                     &client,
                     &api_key,
                     directive,
                     memory.clone(),
                     vmo_state.clone(),
                     tx.clone(),
                     dist.clone()
                 ).await?;\"\"\"

new_block = \"\"\"let ctx = TaskContext {
    orchestrator: orchestrator.clone(),
    client: client.clone(),
    api_key: api_key.clone(),
    directive: directive.to_string(),
    memory: memory.clone(),
    vmo: vmo_state.clone(),
    tx: tx.clone(),
    dist: dist.clone(),
};
let response = send_task(ctx).await?;\"\"\"

content = content.replace(old_block, new_block)
with open(path, 'w') as f:
    f.write(content)
"

echo "Refactor complete. Running verify build..."
cargo check --bin nexus_cli
