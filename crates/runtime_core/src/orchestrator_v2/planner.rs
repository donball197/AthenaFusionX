use crate::orchestrator_v2::task_graph::{TaskExecutionState, TaskGraph, TaskNode, TaskStatus};
use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;

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
            r#"You are the Lead DAG Planner Agent for AthenaFusionX.
Decompose the user DIRECTIVE into a TaskGraph JSON object with a 'nodes' map.
CRITICAL RULE [GRANULARITY]: Create exactly as many nodes as necessary. If the directive is to write or generate a single cohesive document, DO NOT split it into multiple nodes; handle it in a single node to maintain context.
CRITICAL RULE [JSON ESCAPING]: Do NOT use raw newlines inside JSON strings. Use \n.
CRITICAL RULE [ROUTER LOCATION]: The Executive Cortex router is located at 'crates/runtime_core/src/router.rs'. Always use 'crates/runtime_core/src/router.rs' in task objectives when referring to the router.
CRITICAL RULE [DEPENDENCY INFERENCE]: If one task reads, parses, or consumes a file or resource that another task generates, writes, or produces, the consuming task's 'deps' array MUST include the producing task's id. Never schedule a read of a path before the task that creates it.
CRITICAL RULE [TERMINAL STEPS]: A directive step that is purely a control-flow or meta instruction with no tool call and no file output (e.g. "halt", "stop", "done", "finish execution", "end mission") is NOT real work and MUST NOT become its own node. Omit it from the graph entirely -- mission completion is handled automatically once all real work nodes finish.
CRITICAL RULE [WRITE STEPS ARE NEVER TERMINAL]: Any instruction to write, export, generate, or synthesize a file or report (e.g. "use synthesize_report", "write to X.md", "output a document to Y") IS real work and MUST become its own node with the exact output file path stated in its objective, even when the directive immediately follows it with a halt instruction. Never omit a write/output step.
CRITICAL RULE [CONDITIONAL REMEDIATION]: Do not create a separate downstream node whose entire purpose is to "fix", "remediate", or "correct" something that a prior diagnostic node has not yet confirmed exists. When a directive says something like "check X, then fix any issues found" or "run tests and correct all errors", scope that as a SINGLE node (e.g. "check X and fix any issues found") rather than splitting diagnosis and remediation into two nodes -- the outcome depends on what the diagnostic step actually discovers, and a separate node presupposes there is necessarily something to fix.
DIRECTIVE: {}
OUTPUT ONLY VALID JSON with this exact structure:
{{
  "nodes": {{
    "task_1": {{
      "id": "task_1",
      "deps": [],
      "required_agent": "CodeAnalyzer",
      "objective": "..."
    }}
  }}
}}"#,
            directive
        );

        let payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            "generationConfig": { "temperature": 0.1, "maxOutputTokens": 2048 }
        });

        let text_res = crate::epistemic_engine::EpistemicEngine::safe_api_call(
            client,
            api_key,
            &crate::model_config::get_planner_models(),
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
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| {
                        let f_id = format!("task_fallback_{}", uuid::Uuid::new_v4().simple());
                        tracing::warn!(
                            "Missing task ID in graph, auto-generating unique fallback: {}",
                            f_id
                        );
                        f_id
                    });

                let obj = item
                    .get("objective")
                    .or_else(|| item.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(directive)
                    .to_string();

                let lower_obj = obj.trim().to_lowercase();
                if lower_obj == "halt"
                    || lower_obj == "halt."
                    || lower_obj == "halt execution"
                    || lower_obj == "halt execution."
                    || lower_obj.starts_with("halt execution")
                {
                    continue;
                }

                let deps = item
                    .get("deps")
                    .or_else(|| item.get("dependencies"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter().filter_map(|x| x.as_str().map(ToString::to_string)).collect()
                    })
                    .unwrap_or_default();
                let agent = item
                    .get("required_agent")
                    .or_else(|| item.get("agent"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("CodeAnalyzer")
                    .to_string();

                map.insert(
                    id.clone(),
                    TaskNode {
                        id,
                        deps,
                        required_agent: agent,
                        status: TaskStatus::Pending,
                        objective: obj,
                        success_criteria: vec![],
                        execution_state: TaskExecutionState::default(),
                        output_evidence: None,
                        priority: idx + 1,
                    },
                );
            }
            if !map.is_empty() {
                return Ok(TaskGraph { nodes: map });
            }
        }

        Ok(Self::create_fallback_graph(directive))
    }

    fn strip_terminal_nodes(graph: &mut TaskGraph) {
        let terminal_ids: Vec<String> = graph
            .nodes
            .iter()
            .filter(|(_, node)| {
                let obj = node.objective.trim().to_lowercase();
                // ⚡ FIX: Don't strip nodes that include a file path (a write step)
                if obj.contains(".md") || obj.contains(".json") || obj.contains(".rs") {
                    return false;
                }
                obj == "halt"
                    || obj == "halt."
                    || obj == "halt execution"
                    || obj == "halt execution."
                    || obj.starts_with("halt execution")
                    || obj.starts_with("stop execution")
                    || obj.starts_with("end mission")
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in terminal_ids {
            graph.nodes.remove(&id);
        }
    }

    fn create_fallback_graph(directive: &str) -> TaskGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "task_1".to_string(),
            TaskNode {
                id: "task_1".to_string(),
                deps: vec![],
                required_agent: "CodeAnalyzer".to_string(),
                status: TaskStatus::Pending,
                objective: directive.to_string(),
                success_criteria: vec![],
                execution_state: TaskExecutionState::default(),
                output_evidence: None,
                priority: 1,
            },
        );

        TaskGraph { nodes }
    }
}
