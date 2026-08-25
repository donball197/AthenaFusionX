#!/bin/bash
# 1. Add to AgentAction enum
python3 -c "
import os
f = 'crates/runtime_core/src/agent_protocol.rs'
with open(f, 'r') as file: content = file.read()
if 'index_codebase' not in content:
    old = '    #[serde(rename = \"search_code_semantic\")]'
    new = '''    #[serde(rename = \"index_codebase\")]
    IndexCodebase {
        path: String,
        #[serde(default)]
        reasoning: String,
    },
    #[serde(rename = \"search_code_semantic\")]'''
    content = content.replace(old, new)
    with open(f, 'w') as file: file.write(content)
    print('[✅] Added IndexCodebase to agent_protocol.rs')
"

# 2. Add to MotorAction & Mapping
python3 -c "
import os
f = 'crates/runtime_core/src/motor_cortex.rs'
with open(f, 'r') as file: content = file.read()
if 'IndexCodebase' not in content:
    old = '    SearchCodeSemantic { query },'
    new = '''    IndexCodebase { path },
    SearchCodeSemantic { query },'''
    content = content.replace(old, new)
    
    old_map = '            AgentAction::SearchCodeSemantic { query, .. } => MotorAction::SearchCodeSemantic { query },'
    new_map = '''            AgentAction::IndexCodebase { path, .. } => MotorAction::IndexCodebase { path },
            AgentAction::SearchCodeSemantic { query, .. } => MotorAction::SearchCodeSemantic { query },'''
    content = content.replace(old_map, new_map)

    with open(f, 'w') as file: file.write(content)
    print('[✅] Added IndexCodebase to motor_cortex.rs')
"

# 3. Add to Executor (RoboticExecutor)
python3 -c "
import os
f = 'crates/runtime_core/src/executor.rs'
with open(f, 'r') as file: content = file.read()
if 'MotorAction::IndexCodebase' not in content:
    old = '            MotorAction::SearchCodeSemantic { query } => {'
    new = '''            MotorAction::IndexCodebase { path } => {
                // 1. Run the AST planner to generate symbol_graph.json for the target path
                let planner_cmd = format!(\"cargo run --bin repository_planner -- {}\ &\& cargo run --bin symbol_graph\");
                let _ = crate::podman::PodmanShield::execute(&planner_cmd)?;
                
                // 2. Open a DB pool and index the generated graph
                let db_path = \"nexus_daemon.db\";
                match sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect(&format!(\"sqlite://{}\", db_path)).await {
                    Ok(pool) => {
                        match nexus_substrate::code_graph::CodeGraph::index_codebase(&pool, std::path::Path::new(\"repository_analysis/symbol_graph.json\"), &self.api_key).await {
                            Ok(count) => Ok(format!(\"[INDEX_OK] Successfully indexed {} AST nodes from {}\", count, path)),
                            Err(e) => Ok(format!(\"[INDEX_FAIL] {}\", e))
                        }
                    },
                    Err(e) => Ok(format!(\"[INDEX_DB_FAIL] {}\", e))
                }
            }
            MotorAction::SearchCodeSemantic { query } => {'''
    content = content.replace(old, new)
    with open(f, 'w') as file: file.write(content)
    print('[✅] Added IndexCodebase execution logic to executor.rs')
"

# 4. Add to Tool Schema
python3 -c "
import os
f = 'crates/runtime_core/src/tools/schema.rs'
with open(f, 'r') as file: content = file.read()
if 'index_codebase' not in content:
    old = '''        FunctionDeclaration::for_action(
            \"search_code_semantic\",'''
    new = '''        FunctionDeclaration::for_action(
            \"index_codebase\",
            \"Parses a directory, extracts AST nodes, and generates vector embeddings so you can search_code_semantic. Use this after cloning a new repository or if the codebase has changed significantly.\",
            &[\"path\", \"reasoning\"],
            json!({
                \"path\": { \"type\": \"string\", \"description\": \"The directory to index (e.g., './nexus_audit_target')\" },
                \"reasoning\": { \"type\": \"string\" },
            }),
        ),
        FunctionDeclaration::for_action(
            \"search_code_semantic\",'''
    content = content.replace(old, new)
    
    old_parse = '        \"search_code_semantic\" => Ok(AgentAction::SearchCodeSemantic {'
    new_parse = '''        \"index_codebase\" => Ok(AgentAction::IndexCodebase {
            path: get_str(\"path\")?,
            reasoning: get_str(\"reasoning\")?,
        }),
        \"search_code_semantic\" => Ok(AgentAction::SearchCodeSemantic {'''
    content = content.replace(old_parse, new_parse)

    with open(f, 'w') as file: file.write(content)
    print('[✅] Added index_codebase to tools/schema.rs')
"

echo ">> Rebuilding Workspace..."
cargo check -j 4
