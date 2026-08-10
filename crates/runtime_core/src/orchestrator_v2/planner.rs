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
            "You are the Lead DAG Planner Agent for AthenaFusionX.\n\
             Decompose the user DIRECTIVE into a TaskGraph JSON object with a 'nodes' map.\n\
             CRITICAL RULE [GRANULARITY]: Break complex directives into many small, isolated nodes. If asked to fix multiple vulnerabilities or modify multiple files, create at least one distinct node for EACH file/vulnerability.\n\
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
                // FIX: Item #13 Unwrap Masking Error patched with proper tracing fallback
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
