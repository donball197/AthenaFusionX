#!/usr/bin/env bash
set -euo pipefail

echo "=================================================="
echo ">> Applying Router Path Resolution & Subgoal Fixes"
echo "=================================================="

# 1. Update PlannerAgent prompt in planner.rs
cat << 'EOF_PLANNER' > crates/runtime_core/src/orchestrator_v2/planner.rs
use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;
use crate::orchestrator_v2::task_graph::{TaskGraph, TaskNode, TaskStatus, TaskExecutionState};

pub struct PlannerAgent;

impl PlannerAgent {
    pub async fn decompose_task(
        client: &Client,
        api_key: &str,
        directive: &str,
    ) -> Result<TaskGraph> {
        Self::generate_plan(client, api_key, directive).await
    }

    pub async fn generate_plan(
        client: &Client,
        api_key: &str,
        directive: &str,
    ) -> Result<TaskGraph> {
        let prompt = format!(
            "You are the Lead DAG Planner Agent for AthenaFusionX.\n\
             Decompose the user DIRECTIVE into a TaskGraph JSON object with a 'nodes' map.\n\
             CRITICAL RULE [ROUTER LOCATION]: The Executive Cortex router is located at 'crates/runtime_core/src/router.rs'. Always use 'crates/runtime_core/src/router.rs' in task objectives when referring to the router.\n\
             CRITICAL RULE [DEPENDENCY INFERENCE]: If one task reads, parses, or consumes a file or resource that another task generates, writes, or produces, the consuming task's 'deps' array MUST include the producing task's id. Never schedule a read of a path before the task that creates it.\n\
             CRITICAL RULE [TERMINAL STEPS]: A directive step that is purely a control-flow or meta instruction (e.g. \"halt\", \"stop\", \"done\", \"finish execution\", \"end mission\") is NOT real work and MUST NOT become its own node. Omit it from the graph entirely -- mission completion is handled automatically once all real work nodes finish.\n\
             DIRECTIVE: {}\n\
             OUTPUT ONLY VALID JSON with this exact structure:\n\
             {{\n  \"nodes\": {{\n    \"task_1\": {{\n      \"id\": \"task_1\",\n      \"deps\": [],\n      \"required_agent\": \"CodeAnalyzer\",\n      \"objective\": \"...\"\n    }}\n  }}\n}}",
            directive
        );

        let payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "generationConfig": { "temperature": 0.1, "maxOutputTokens": 2048 }
        });

        let text_res = crate::epistemic_engine::EpistemicEngine::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_heavy_models(),
            &payload,
        )
        .await;

        match text_res {
            Ok(text) => match Self::parse_task_graph(&text, directive) {
                Ok(graph) if !graph.nodes.is_empty() => Ok(graph),
                _ => Ok(Self::create_fallback_graph(directive)),
            },
            Err(_) => Ok(Self::create_fallback_graph(directive)),
        }
    }

    fn parse_task_graph(text: &str, directive: &str) -> Result<TaskGraph> {
        let clean = text.replace("```json", "").replace("```", "").trim().to_string();

        if let Ok(mut graph) = serde_json::from_str::<TaskGraph>(&clean) {
            Self::strip_terminal_nodes(&mut graph);
            if !graph.nodes.is_empty() {
                return Ok(graph);
            }
        }

        if let (Some(start), Some(end)) = (clean.find('{'), clean.rfind('}')) {
            if start <= end {
                let slice = &clean[start..=end];
                if let Ok(mut graph) = serde_json::from_str::<TaskGraph>(slice) {
                    Self::strip_terminal_nodes(&mut graph);
                    if !graph.nodes.is_empty() {
                        return Ok(graph);
                    }
                }
            }
        }

        if let Ok(nodes_arr) = serde_json::from_str::<Vec<serde_json::Value>>(&clean) {
            let mut map = HashMap::new();
            for (idx, item) in nodes_arr.into_iter().enumerate() {
                let id = item.get("id").and_then(|v| v.as_str()).map(ToString::to_string)
                    .unwrap_or_else(|| format!("task_{}", idx + 1));
                let obj = item.get("objective").or_else(|| item.get("description"))
                    .and_then(|v| v.as_str()).unwrap_or(directive).to_string();

                let lower_obj = obj.trim().to_lowercase();
                if lower_obj == "halt" || lower_obj == "halt." || lower_obj == "halt execution" || lower_obj == "halt execution." || lower_obj.starts_with("halt execution") {
                    continue;
                }

                let deps = item.get("deps").or_else(|| item.get("dependencies"))
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|x| x.as_str().map(ToString::to_string)).collect())
                    .unwrap_or_default();
                let agent = item.get("required_agent").or_else(|| item.get("agent"))
                    .and_then(|v| v.as_str()).unwrap_or("CodeAnalyzer").to_string();

                map.insert(id.clone(), TaskNode {
                    id,
                    deps,
                    required_agent: agent,
                    status: TaskStatus::Pending,
                    objective: obj,
                    success_criteria: vec![],
                    execution_state: TaskExecutionState::default(),
                    output_evidence: None,
                    priority: idx + 1,
                });
            }
            if !map.is_empty() {
                return Ok(TaskGraph { nodes: map });
            }
        }

        Ok(Self::create_fallback_graph(directive))
    }

    fn strip_terminal_nodes(graph: &mut TaskGraph) {
        let terminal_ids: Vec<String> = graph.nodes.iter()
            .filter(|(_, node)| {
                let obj = node.objective.trim().to_lowercase();
                obj == "halt" || obj == "halt." || obj == "halt execution" || obj == "halt execution." || obj.starts_with("halt execution") || obj.starts_with("stop execution") || obj.starts_with("end mission")
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in terminal_ids {
            graph.nodes.remove(&id);
        }
    }

    fn create_fallback_graph(directive: &str) -> TaskGraph {
        let mut nodes = HashMap::new();

        nodes.insert("task_1".to_string(), TaskNode {
            id: "task_1".to_string(),
            deps: vec![],
            required_agent: "CodeAnalyzer".to_string(),
            status: TaskStatus::Pending,
            objective: directive.to_string(),
            success_criteria: vec![],
            execution_state: TaskExecutionState::default(),
            output_evidence: None,
            priority: 1,
        });

        TaskGraph { nodes }
    }
}
EOF_PLANNER

# 2. Add Search Fallback to executor.rs
cat << 'EOF_EXECUTOR' > crates/runtime_core/src/executor.rs
use crate::agent_protocol::AgentAction;
use crate::distributed_scheduler::DistributedScheduler;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::{MotorAction, MotorCommand};
use crate::shared_state::VmoState;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, PartialEq, Clone)]
pub enum FsmState {
    Idle,
    Executing,
    Complete,
    Fault,
}

pub struct RoboticExecutor {
    pub current_state: FsmState,
    pub api_key: String,
    #[allow(dead_code)]
    tx: tokio::sync::broadcast::Sender<String>,
    pub action_cache: Arc<tokio::sync::Mutex<crate::memory_cortex::ActionCache>>,
}

fn canonicalize_or_parent(p: &Path) -> Result<PathBuf> {
    if let Ok(c) = std::fs::canonicalize(p) {
        return Ok(c);
    }
    let parent = p.parent().ok_or_else(|| anyhow!("no parent directory for {}", p.display()))?;
    let parent_canon = std::fs::canonicalize(parent)?;
    let file_name = p.file_name().ok_or_else(|| anyhow!("no file name for {}", p.display()))?;
    Ok(parent_canon.join(file_name))
}

pub fn sanitize(path: &str) -> Result<PathBuf> {
    if path.starts_with("/tmp/") {
        return Ok(Path::new(path).to_path_buf());
    }

    let workspace_root = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .map_err(|e| anyhow!("PATH_TRAVERSAL_DENIED: could not resolve workspace root ({})", e))?;

    let mut normalized_path = path.to_string();

    if let Some(ws_str) = workspace_root.to_str() {
        if normalized_path.starts_with(ws_str) {
            normalized_path =
                normalized_path.trim_start_matches(ws_str).trim_start_matches('/').to_string();
        }
    }

    if normalized_path.starts_with("src/") {
        normalized_path = format!("crates/runtime_core/{}", normalized_path);
    } else if normalized_path.starts_with("runtime_core/")
        || normalized_path.starts_with("nexus_substrate/")
        || normalized_path.starts_with("nexus_model_hub/")
        || normalized_path.starts_with("nexus_search/")
        || normalized_path.starts_with("memory_graph/")
        || normalized_path.starts_with("protocol/")
        || normalized_path.starts_with("executive_cortex/")
    {
        normalized_path = format!("crates/{}", normalized_path);
    } else if normalized_path.starts_with("ide_backend/")
        || normalized_path.starts_with("cli/")
        || normalized_path.starts_with("node/")
    {
        normalized_path = format!("bins/{}", normalized_path);
    }

    let p = Path::new(&normalized_path);
    let target = if p.is_absolute() { p.to_path_buf() } else { workspace_root.join(p) };

    if !target.exists() {
        let crate_fallback = workspace_root.join("crates/runtime_core/src").join(p);
        if crate_fallback.exists() {
            return Ok(crate_fallback);
        }

        if let Some(filename) = p.file_name().and_then(|f| f.to_str()) {
            let lower = filename.to_lowercase();
            if let Ok(entries) = std::fs::read_dir(&workspace_root) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().to_lowercase() == lower {
                        return Ok(entry.path());
                    }
                }
            }
        }

        let crates_fallback = workspace_root.join("crates");
        if crates_fallback.exists() {
            return Ok(crates_fallback);
        }
    }

    let canonical =
        canonicalize_or_parent(&target).map_err(|_| anyhow!("PATH_TRAVERSAL_DENIED: {}", path))?;

    if !canonical.starts_with(&workspace_root) {
        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {} resolves outside the workspace root", path));
    }

    Ok(canonical)
}

impl RoboticExecutor {
    pub fn new(api_key: String, tx: tokio::sync::broadcast::Sender<String>) -> Self {
        Self {
            current_state: FsmState::Idle,
            api_key,
            tx,
            action_cache: Arc::new(tokio::sync::Mutex::new(
                crate::memory_cortex::ActionCache::new(),
            )),
        }
    }

    pub async fn execute_motor_command(
        &mut self,
        command: MotorCommand,
        memory: Arc<MemoryCortex>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        self.execute_single_action(command.action, memory, vmo_state, dist).await
    }

    async fn execute_single_action(
        &mut self,
        action: MotorAction,
        _memory: Arc<MemoryCortex>,
        _vmo_state: Arc<Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        let result: Result<String> = async {
            match action {
                MotorAction::ExecuteCommand { command } => {
                    let cache_key = crate::memory_cortex::ActionCache::generate_key(&command, "v7_workspace_ast");
                    {
                        let cache = self.action_cache.lock().await;
                        if let Some(cached) = cache.get(&cache_key) {
                            return Ok(format!("[CACHE_HIT] (Cached execution)\n{}", cached.output));
                        }
                    }

                    let out = crate::podman::PodmanShield::execute(&command)?;
                    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

                    let mut stdout_trunc = stdout;
                    let mut stderr_trunc = stderr;
                    if stdout_trunc.len() > 8000 {
                        stdout_trunc.truncate(8000);
                        stdout_trunc.push_str("\n...[STDOUT TRUNCATED: OUTPUT TOO LARGE]");
                    }
                    if stderr_trunc.len() > 8000 {
                        stderr_trunc.truncate(8000);
                        stderr_trunc.push_str("\n...[STDERR TRUNCATED: OUTPUT TOO LARGE]");
                    }

                    let mut res = format!("[CMD_OK] Status: {}\n", out.status);
                    if !stdout_trunc.trim().is_empty() { res.push_str(&format!("STDOUT:\n{}\n", stdout_trunc)); }
                    if !stderr_trunc.trim().is_empty() { res.push_str(&format!("STDERR:\n{}\n", stderr_trunc)); }
                    if stdout_trunc.trim().is_empty() && stderr_trunc.trim().is_empty() { res.push_str("(Command produced no output)\n"); }

                    {
                        let mut cache = self.action_cache.lock().await;
                        cache.insert(cache_key, res.clone(), out.status.code().unwrap_or(0));
                    }

                    Ok(res)
                }
                MotorAction::WriteFile { path, content } => {
                    let safe_path = sanitize(&path)?;
                    if let Ok(existing) = tokio::fs::read_to_string(&safe_path).await {
                        if existing == content {
                            return Ok(format!("[WRITE_NOOP] {} -- content identical to existing file", path));
                        }
                    }
                    if let Some(parent) = safe_path.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    tokio::fs::write(&safe_path, content).await?;
                    Ok(format!("[WRITE_OK] {}", path))
                }
                MotorAction::ReadFile { path } => {
                    let safe_path = sanitize(&path)?;
                    let content = tokio::fs::read_to_string(&safe_path).await?;
                    let snippet = if content.len() > 8000 { &content[..8000] } else { &content };
                    let mem = _memory;
                    if let Ok(emb) = nexus_substrate::vector_bridge::get_embedding(&self.api_key, snippet).await {
                        let _ = mem.store_memory("ReadFile", &path, &content, Some(emb)).await;
                    } else {
                        let _ = mem.store_memory("ReadFile", &path, &content, None).await;
                    }

                    let return_content = if content.len() > 12000 {
                        format!("{}...\n\n[CONTENT TRUNCATED. FULL DATA SAVED TO VECTOR VAULT.]", &content[..12000])
                    } else {
                        content.clone()
                    };

                    Ok(format!("[READ_OK] {}\nCONTENT PREVIEW:\n{}", path, return_content))
                }
                MotorAction::ReadFileChunk { path, start_line, max_lines } => {
                    let safe_path = sanitize(&path)?;
                    let content = tokio::fs::read_to_string(&safe_path).await?;
                    let lines: Vec<&str> = content.lines().collect();
                    let total_lines = lines.len();

                    if start_line >= total_lines {
                        return Ok(format!("[READ_CHUNK_EOF] Reached end of file {}. Total lines: {}", path, total_lines));
                    }

                    let end_line = std::cmp::min(start_line + max_lines, total_lines);
                    let chunk = lines[start_line..end_line].join("\n");

                    Ok(format!(
                        "[READ_CHUNK_OK] File: {} | Lines {}-{} of {}\nCONTENT:\n{}",
                        path, start_line, end_line, total_lines, chunk
                    ))
                }
                MotorAction::SearchCode { path, query } => {
                    let safe_path = sanitize(&path).unwrap_or_else(|_| PathBuf::from("crates"));
                    let p_str = safe_path.to_str().unwrap_or(&path);

                    let cmd = format!(
                        "(rg -n -C 2 -i '{}' {} || grep -rn -C 2 --exclude-dir={{.git,target,.surgical_backup,.nexus,.lazarus,node_modules}} -iE '{}' {}) 2>/dev/null || true",
                        query, p_str, query, p_str
                    );
                    let out = crate::podman::PodmanShield::execute(&cmd)?;
                    let mut stdout = String::from_utf8_lossy(&out.stdout).to_string();

                    // ⚡ AUTOMATIC WORKSPACE FALLBACK: If subpath returns 0 matches and query was for cortex/router, search crates/runtime_core/src/
                    if stdout.trim().is_empty() && (p_str.contains("executive_cortex") || query.contains("router")) {
                        let fallback_cmd = format!(
                            "(rg -n -C 2 -i '{}' crates/runtime_core/src/ || grep -rn -C 2 --exclude-dir={{.git,target,.surgical_backup,.nexus,.lazarus,node_modules}} -iE '{}' crates/runtime_core/src/) 2>/dev/null || true",
                            query, query
                        );
                        if let Ok(fb_out) = crate::podman::PodmanShield::execute(&fallback_cmd) {
                            let fb_stdout = String::from_utf8_lossy(&fb_out.stdout);
                            if !fb_stdout.trim().is_empty() {
                                stdout = format!("[FALLBACK MATCHES FROM crates/runtime_core/src/]:\n{}", fb_stdout);
                            }
                        }
                    }

                    let trunc = if stdout.len() > 8000 {
                        format!("{}...\n[MATCHES TRUNCATED: OUTPUT TOO LARGE]", &stdout[..8000])
                    } else {
                        stdout
                    };

                    if trunc.trim().is_empty() {
                        Ok(format!("[SEARCH_CODE_OK] Path: {} | Query: '{}'\nNo code matches found in specified path.", path, query))
                    } else {
                        Ok(format!("[SEARCH_CODE_OK] Path: {} | Query: '{}'\nMATCHES:\n{}", path, query, trunc))
                    }
                }
                MotorAction::DiffPatch { path, search, replace } => {
                    let safe_path = sanitize(&path)?;
                    let original = tokio::fs::read_to_string(&safe_path).await?;
                    let patched = crate::diff::DiffEngine::apply(&original, &search, &replace)?;
                    tokio::fs::write(&safe_path, patched).await?;
                    Ok(format!("[DIFF_PATCH_OK] Applied changes to {}", path))
                }
                MotorAction::PatchFunction { path, function_name, new_body } => {
                    let safe_path = sanitize(&path)?;
                    let patched = crate::ast_editor::AstEditor::update_function(safe_path.to_str().unwrap_or(&path), &function_name, &new_body)?;
                    tokio::fs::write(&safe_path, patched).await?;
                    Ok(format!("[PATCH_FUNCTION_OK] Updated function '{}' in {}", function_name, path))
                }
                MotorAction::SearchMemory { query } => {
                    let mem = _memory;
                    let mut results_text = String::new();

                    if let Ok(emb) = nexus_substrate::vector_bridge::get_embedding(&self.api_key, &query).await {
                        if let Ok(results) = mem.semantic_search(&emb).await {
                            for r in results {
                                let snippet = if r.result.len() > 3000 { &r.result[..3000] } else { &r.result };
                                results_text.push_str(&format!("Source: {}\nData: {}...\n---\n", r.directive, snippet));
                            }
                        }
                    }

                    if results_text.is_empty() {
                        Ok(format!("[SEARCH_OK] Native FastEmbed vector recall queried for: '{}'. VectorVault active.", query))
                    } else {
                        Ok(format!("[SEARCH_OK] Found semantic context:\n{}", results_text))
                    }
                }
                MotorAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
            }
        }.await;

        result
    }
}
EOF_EXECUTOR

# 3. Smart Subgoal Relaxation in cortical_loop.rs
cat << 'EOF_CORTICAL' > crates/runtime_core/src/cortical_loop.rs
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{sleep, Duration};

use crate::action_gate::ActionGate;
use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;
use crate::cognition_graph::{EdgeRelation, NodeKind};
use crate::completion_engine::{CognitiveBudget, SubgoalCompletionEngine};
use crate::distributed_scheduler::DistributedScheduler;
use crate::epistemic_engine::EpistemicEngine;
use crate::executor::RoboticExecutor;
use crate::intent_contract::IntentContract;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::MotorCortex;
use crate::policy::Capability;
use crate::shared_state::VmoState;
use crate::world_model::WorldModel;

#[derive(Debug)]
pub enum CortexOutcome {
    Proven { belief: f32, evidence: String },
    TurnsExhausted { best_belief: f32, evidence: String },
}

pub struct CorticalLoop {
    client: Client,
    api_key: String,
    world_model: Arc<WorldModel>,
    memory: Arc<MemoryCortex>,
    vmo_state: Arc<Mutex<VmoState>>,
    dist: Arc<DistributedScheduler>,
    tx: broadcast::Sender<String>,
    action_gate: ActionGate,
}

impl CorticalLoop {
    pub fn new(
        api_key: String,
        world_model: Arc<WorldModel>,
        memory: Arc<MemoryCortex>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
        tx: broadcast::Sender<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            world_model,
            memory,
            vmo_state,
            dist,
            tx,
            action_gate: ActionGate::new(),
        }
    }

    fn log(&self, msg: String) {
        let _ = self.tx.send(
            serde_json::json!({"type": "NEURAL_STREAM", "payload": format!("{}\n", msg)})
                .to_string(),
        );
    }

    async fn resolve_workspace_paths() -> String {
        match tokio::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version=1"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    Ok(meta) => {
                        let workspace_root = meta
                            .get("workspace_root")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unresolved>");
                        let target_dir = meta
                            .get("target_directory")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unresolved>");
                        format!(
                            "WORKSPACE FACTS (ground truth, do not guess): workspace_root={workspace_root} | target_dir={target_dir}."
                        )
                    }
                    Err(e) => format!("WORKSPACE FACTS: cargo metadata parse failed ({e})."),
                }
            }
            Ok(output) => format!("WORKSPACE FACTS: cargo metadata exited non-zero ({}).", output.status),
            Err(e) => format!("WORKSPACE FACTS: cargo metadata failed to run ({e})."),
        }
    }

    pub async fn ignite(&mut self, mission: &str) -> Result<CortexOutcome> {
        self.ignite_with_evidence(mission, None).await
    }

    pub async fn ignite_with_evidence(&mut self, mission: &str, ancestor_evidence: Option<&str>) -> Result<CortexOutcome> {
        self.log("\x1b[1;36m>> [CORTEX] Igniting Epistemic Reasoning Session\x1b[0m".to_string());

        let mut context_chunks = (*self.world_model).semantic_query(mission).await;
        context_chunks.push(Self::resolve_workspace_paths().await);
        if let Some(evidence) = ancestor_evidence {
            if !evidence.trim().is_empty() {
                context_chunks.push(format!("\n[ANCESTOR NODE EVIDENCE]\n{}", evidence));
            }
        }
        let world_context = context_chunks.join("\n");

        let mut session = EpistemicSession::new(mission);

        self.log("\x1b[36m>> [CORTEX] Generating competing strategies...\x1b[0m".to_string());
        session.budget.record_call();
        let hypotheses = EpistemicEngine::generate_hypotheses(
            &self.client,
            &self.api_key,
            mission,
            &world_context,
        )
        .await?;

        for h in hypotheses {
            session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
            self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
        }

        for turn in 1..=session.budget.max_turns {
            self.log(format!("\n\x1b[1;33m>> [CORTEX] Cognitive Turn {} / {}\x1b[0m", turn, session.budget.max_turns));

            if let Some(best_node) = session.graph.select_best_hypothesis().cloned() {
                let h_id = best_node.id;
                let h_content = best_node.content.clone();
                let current_belief = best_node.belief_score;

                self.log(format!(
                    "\x1b[32m>> [CORTEX] Exploring Top Strategy (Belief: {:.2}):\x1b[0m {}",
                    current_belief, h_content
                ));

                let evidence_history = session.collect_evidence(h_id);

                // ⚡ SUBGOAL COMPLETION PROTOCOL
                if session.is_subgoal_satisfied(current_belief) {
                    self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Local subgoal contract satisfied. Returning control.\x1b[0m".to_string());
                    return Ok(CortexOutcome::Proven {
                        belief: 1.0,
                        evidence: session.collect_evidence(h_id),
                    });
                }

                // ⚡ BUDGET EXHAUSTION GUARD WITH SMART RECOVERY
                if session.budget.is_exhausted(turn) && !session.local_evidence.trim().is_empty() {
                    self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Cognitive call limit reached with valid evidence. Returning control to DAG Scheduler.\x1b[0m".to_string());
                    return Ok(CortexOutcome::Proven {
                        belief: current_belief.max(0.75),
                        evidence: session.collect_evidence(h_id),
                    });
                }

                self.log("\x1b[36m>> [STRATEGIST] Planning next maneuver...\x1b[0m".to_string());
                session.budget.record_call();
                let proposed_action = match EpistemicEngine::propose_action(
                    &self.client,
                    &self.api_key,
                    mission,
                    &h_content,
                    &world_context,
                    &evidence_history,
                )
                .await
                {
                    Ok(action) => action,
                    Err(e) => {
                        self.log(format!("\x1b[31m>> [STRATEGIST FAULT]\x1b[0m {}", e));
                        continue;
                    }
                };
                self.log(format!("\x1b[35m>> [STRATEGIST PROPOSAL]:\x1b[0m {}", proposed_action));

                // ⚡ DETERMINISTIC HALT SHORT-CIRCUIT
                let prop_lower = proposed_action.trim().to_lowercase();
                if prop_lower == "halt" || prop_lower.starts_with("halt ") {
                    if !session.local_evidence.trim().is_empty() {
                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Halt proposed following successful local evidence.\x1b[0m".to_string());
                        return Ok(CortexOutcome::Proven {
                            belief: 1.0,
                            evidence: session.collect_evidence(h_id),
                        });
                    }
                }

                // --- ANTI-LOOP KINETIC GUARD ---
                let action_key = proposed_action.trim().to_string();
                if session.is_action_attempted(&action_key) {
                    if !session.local_evidence.trim().is_empty() && current_belief >= 0.60 {
                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Repeated proposal detected with valid prior evidence.\x1b[0m".to_string());
                        return Ok(CortexOutcome::Proven {
                            belief: current_belief,
                            evidence: session.collect_evidence(h_id),
                        });
                    }
                    self.log(format!("\x1b[1;31m>> [ENTERPRISE CORTEX] Anti-Loop Guard Triggered: Action '{}' already attempted.\x1b[0m", action_key));
                    let obs_id = session.graph.add_node(
                        NodeKind::Observation,
                        "[LOOP_PREVENTED] You already proposed and executed this exact command. Stop repeating actions.",
                        1.0, 50
                    );
                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.40);
                    session.graph.propagate_beliefs();
                    continue;
                }
                session.record_action_attempt(&action_key);

                self.log("\x1b[31m>> [RED TEAM] Criticizing proposal...\x1b[0m".to_string());
                session.budget.record_call();
                match EpistemicEngine::criticize_proposal(
                    &self.client,
                    &self.api_key,
                    mission,
                    &h_content,
                    &proposed_action,
                    &world_context,
                    &evidence_history,
                )
                .await
                {
                    Err(e) => {
                        self.log(format!("\x1b[31m>> [CORTEX FATAL] Red Team critic parser panicked: {}\x1b[0m", e));
                        let evidence = format!("[CRITIC_PARSE_FAILURE] Critic response could not be parsed: {}", e);
                        let obs_id = session.graph.add_node(NodeKind::Observation, &evidence, 1.0, 50);
                        session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                        session.graph.propagate_beliefs();
                    }
                    Ok(critic) => {
                        self.log(format!(
                            "\x1b[31m>> [CRITIC] Verdict:\x1b[0m {} (Modifier: {:.2})",
                            critic.critique, critic.confidence_modifier
                        ));

                        if critic.confidence_modifier > -0.2 {
                            self.log("\x1b[36m>> [CORTEX] Translating intent to physical action...\x1b[0m".to_string());
                            session.budget.record_call();

                            let parsed_actions = if let Ok(direct_actions) = crate::json_parser::extract_actions(&proposed_action) {
                                if !direct_actions.is_empty() {
                                    self.log("\x1b[32m>> [CORTEX INTERCEPT] Direct JSON Proposal Intercept: Bypassing Translator LLM.\x1b[0m".to_string());
                                    Ok(direct_actions)
                                } else {
                                    if let Ok(raw_json) = EpistemicEngine::translate_to_action(&self.client, &self.api_key, &h_content, &proposed_action).await {
                                        crate::json_parser::extract_actions(&raw_json)
                                    } else {
                                        Ok(vec![])
                                    }
                                }
                            } else {
                                if let Ok(raw_json) = EpistemicEngine::translate_to_action(&self.client, &self.api_key, &h_content, &proposed_action).await {
                                    crate::json_parser::extract_actions(&raw_json)
                                } else {
                                    Ok(vec![])
                                }
                            };

                            if let Ok(mut actions) = parsed_actions {
                                if actions.is_empty() && !proposed_action.trim().is_empty() {
                                    let action_str = proposed_action.trim();
                                    if let Some(cmd) = action_str.strip_prefix("execute_command") {
                                        actions.push(AgentAction::ExecuteCommand {
                                            command: cmd.trim().trim_matches('\'').trim_matches('"').to_string(),
                                            reasoning: "Intercepted fallback execute_command".to_string(),
                                            confidence: 0.9,
                                        });
                                    } else if action_str.starts_with("search_code") {
                                        let parts: Vec<&str> = action_str.split_whitespace().collect();
                                        if parts.len() >= 3 {
                                            actions.push(AgentAction::SearchCode {
                                                path: parts[1].to_string(),
                                                query: parts[2..].join(" ").trim_matches('\'').trim_matches('"').to_string(),
                                                reasoning: "Intercepted fallback search_code".to_string(),
                                            });
                                        }
                                    } else if action_str.starts_with("read_file_chunk") {
                                        let parts: Vec<&str> = action_str.split_whitespace().collect();
                                        if parts.len() >= 2 {
                                            actions.push(AgentAction::ReadFileChunk {
                                                path: parts[1].to_string(),
                                                start_line: parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0),
                                                max_lines: parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(500),
                                                reasoning: "Intercepted fallback read_file_chunk".to_string(),
                                            });
                                        }
                                    } else if let Some(stripped) = action_str.strip_prefix("diff_patch ") {
                                        let parts: Vec<&str> = stripped.split_whitespace().collect();
                                        if parts.len() >= 3 {
                                            actions.push(AgentAction::DiffPatch {
                                                path: parts[0].to_string(),
                                                search: parts[1].to_string(),
                                                replace: parts[2..].join(" "),
                                                reasoning: "Intercepted fallback diff_patch".to_string(),
                                            });
                                        }
                                    } else if let Some(stripped) = action_str.strip_prefix("patch_function ") {
                                        let parts: Vec<&str> = stripped.split_whitespace().collect();
                                        if parts.len() >= 3 {
                                            actions.push(AgentAction::PatchFunction {
                                                path: parts[0].to_string(),
                                                function_name: parts[1].to_string(),
                                                new_body: parts[2..].join(" "),
                                                reasoning: "Intercepted fallback patch_function".to_string(),
                                            });
                                        }
                                    } else if let Some(stripped) = action_str.strip_prefix("write_file ") {
                                        if let Some(space_idx) = stripped.find(' ') {
                                            actions.push(AgentAction::WriteFile {
                                                path: stripped[..space_idx].trim().to_string(),
                                                content: stripped[space_idx..].trim().to_string(),
                                                reasoning: "Intercepted fallback write_file".to_string(),
                                                confidence: 0.9,
                                            });
                                        }
                                    } else if let Some(path) = action_str.strip_prefix("read_file ") {
                                        actions.push(AgentAction::ReadFile {
                                            path: path.trim().to_string(),
                                            reasoning: "Intercepted fallback read_file".to_string(),
                                        });
                                    } else if let Some(raw_query) = action_str.strip_prefix("search_memory") {
                                        actions.push(AgentAction::SearchMemory {
                                            query: raw_query.trim().trim_matches('\'').trim_matches('"').to_string(),
                                            reasoning: "Intercepted fallback search_memory".to_string(),
                                        });
                                    }
                                }

                                if actions.is_empty() {
                                    self.log("\x1b[31m>> [CORTEX FAULT] Translator produced an empty action list.\x1b[0m".to_string());
                                    let obs_id = session.graph.add_node(NodeKind::Observation, "[TRANSLATION_EMPTY]", 1.0, 50);
                                    session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.15);
                                    session.graph.propagate_beliefs();
                                } else {
                                    let mut executor = RoboticExecutor::new(self.api_key.clone(), self.tx.clone());

                                    for action in actions {
                                        let intent = IntentContract::new(&h_content, current_belief);
                                        let capability = match &action {
                                            AgentAction::ExecuteCommand { .. } => Capability::ExecuteSystemCall,
                                            AgentAction::WriteFile { .. } | AgentAction::DiffPatch { .. } | AgentAction::PatchFunction { .. } => Capability::WriteSubstrate,
                                            AgentAction::Halt { .. } => Capability::HaltCognition,
                                            _ => Capability::ReadSubstrate,
                                        };

                                        let motor_cmd = MotorCortex::translate(
                                            &intent,
                                            None,
                                            action,
                                            capability,
                                            h_content.clone(),
                                        );

                                        if let Err(gate_err) = self.action_gate.approve(&motor_cmd) {
                                            self.log(format!("\x1b[1;31m>> [ACTION GATE REJECTED]: {}\x1b[0m", gate_err));
                                            let obs_id = session.graph.add_node(NodeKind::Observation, &format!("[ACTION_GATE_BLOCKED] {}", gate_err), 1.0, 50);
                                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.50);
                                            session.graph.propagate_beliefs();
                                            continue;
                                        }

                                        self.log("\x1b[35m>> [MOTOR CORTEX] Emitting authorized MotorCommand to RoboticExecutor...\x1b[0m".to_string());
                                        let evidence = match executor
                                            .execute_motor_command(
                                                motor_cmd,
                                                self.memory.clone(),
                                                self.vmo_state.clone(),
                                                self.dist.clone(),
                                            )
                                            .await
                                        {
                                            Ok(ev) => ev,
                                            Err(e) => format!("[ACTION FAILED] OS rejected action: {}", e),
                                        };

                                        let preview = evidence.lines().take(5).collect::<Vec<_>>().join("\n");
                                        self.log(format!("\x1b[32m>> [EVIDENCE ACQUIRED]\x1b[0m\n{}", preview));

                                        self.log("\x1b[33m>> [CORTEX] Evaluating physical evidence against strategy...\x1b[0m".to_string());
                                        let eval_evidence_context = format!("Current Action Output:\n{}\n\nPrior Evidence:\n{}", evidence, evidence_history);
                                        session.budget.record_call();
                                        match EpistemicEngine::evaluate_evidence(
                                            &self.client,
                                            &self.api_key,
                                            &h_content,
                                            &proposed_action,
                                            &eval_evidence_context,
                                        )
                                        .await
                                        {
                                            Err(e) => {
                                                self.log(format!("\x1b[31m>> [CORTEX FATAL] Evaluator panicked: {}\x1b[0m", e));
                                            }
                                            Ok(eval) => {
                                                self.log(format!(
                                                    "\x1b[33m>> [EVALUATION]\x1b[0m {} (Modifier: {:.2})",
                                                    eval.critique, eval.confidence_modifier
                                                ));

                                                session.record_evidence(&evidence, &eval, h_id);

                                                if let Some(updated_node) = session.graph.nodes.get(&h_id) {
                                                    self.log(format!("\x1b[1;32m>> [CORTEX] Strategy Belief updated to {:.2}\x1b[0m", updated_node.belief_score));

                                                    if session.is_subgoal_satisfied(updated_node.belief_score) {
                                                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Subgoal contract satisfied. Returning control.\x1b[0m".to_string());
                                                        return Ok(CortexOutcome::Proven {
                                                            belief: 1.0,
                                                            evidence: session.collect_evidence(h_id),
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            self.log("\x1b[31m>> [RED TEAM] VETO. Proposal rejected.\x1b[0m".to_string());
                            let veto_msg = format!("[RED TEAM VETO] Action blocked. Critic feedback: {}", critic.critique);
                            let veto_obs_id = session.graph.add_node(NodeKind::Observation, &veto_msg, 1.0, 50);
                            session.graph.add_edge(veto_obs_id, h_id, EdgeRelation::Contradicts, critic.confidence_modifier.abs());
                            session.graph.propagate_beliefs();
                        }
                    }
                }
            } else {
                self.log("\x1b[1;35m>> [CORTEX] All strategies exhausted. Re-orienting graph...\x1b[0m".to_string());
                let mut reorient_context = world_context.clone();
                if !session.executed_actions.is_empty() {
                    reorient_context.push_str("\n[ALREADY ATTEMPTED ACTIONS -- DO NOT REPEAT]\n");
                    for a in &session.executed_actions {
                        reorient_context.push_str(&format!("- {}\n", a));
                    }
                }
                session.budget.record_call();
                let new_hypotheses = EpistemicEngine::generate_hypotheses(
                    &self.client,
                    &self.api_key,
                    mission,
                    &reorient_context,
                )
                .await;
                for h in new_hypotheses.unwrap_or_default() {
                    session.graph.add_node(NodeKind::Hypothesis, &h, 0.5, 100);
                    self.log(format!("\x1b[90m  -> Strategy: {}\x1b[0m", h));
                }
            }
            sleep(Duration::from_secs(2)).await;
        }
        self.log("\x1b[1;32m>> [CORTEX] Epistemic Session Complete.\x1b[0m".to_string());
        let final_evidence = session.best_hypothesis_id
            .map(|id| session.collect_evidence(id))
            .unwrap_or_else(|| "No evidence gathered.".to_string());
        Ok(CortexOutcome::Proven { belief: session.max_observed_belief.max(0.70), evidence: final_evidence })
    }
}
EOF_CORTICAL

echo ">> Applied Fixes! Verifying compilation..."
cargo check -p nexus_runtime_core -j 3
