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
        // EXPLICIT NATIVE TOOL CONTRACT:
        // An explicit SearchMemory directive must never be reinterpreted
        // by the planner model as SearchCode, ReadFile, or filesystem work.
        let search_memory_explicit =
            crate::completion_engine::any_word_unnegated(directive, &["searchmemory"])
                || crate::completion_engine::any_word_unnegated(directive, &["search_memory"])
                || (crate::completion_engine::any_word_unnegated(directive, &["search"])
                    && crate::completion_engine::any_word_unnegated(directive, &["memory"]));

        if search_memory_explicit {
            return Ok(Self::create_search_memory_graph(directive));
        }

        let prompt = format!(
            r#"You are the Lead DAG Planner Agent for AthenaFusionX.
Decompose the user DIRECTIVE into a TaskGraph JSON object with a 'nodes' map.
CRITICAL RULE [GRANULARITY]: Create exactly as many nodes as necessary. If the directive is to write or generate a single cohesive document, DO NOT split it into multiple nodes; handle it in a single node to maintain context.
CRITICAL RULE [SUCCESS CONTRACT]: EVERY real work node MUST contain a non-empty 'success_criteria' array. Each criterion MUST be deterministic and physically checkable from execution evidence. Use exact forms such as:
- "READ_OK:crates/runtime_core/src/router.rs"
- "READ_FUNCTION_OK:crates/runtime_core/src/router.rs:route"
- "SEARCH_CODE_OK:model configuration"
- "WRITE_OK:model_routing_analysis.md"
- "CMD_OK"
- "PATCH_FUNCTION_OK:path/to/file.rs:function_name"
- "DIFF_PATCH_OK:path/to/file.rs"
Never use vague criteria such as "analysis complete", "understand the code", or "agent is satisfied".
CRITICAL RULE [MULTI-STEP OBJECTIVES]: If a node requires multiple distinct investigations, include one explicit criterion for each required evidence type/path/query. Every criterion must be satisfied before the node can be completed.
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
      "objective": "...",
      "success_criteria": ["CMD_OK"]
    }}
  }}
}}"#,
            directive
        );

        let payload = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": prompt}]}],
            // Deterministic decomposition: the same directive must produce
            // the same DAG shape across runs. Confirmed source of the
            // 12-node/1-node divergence on identical audit prompts. The
            // cortex handles narrow nodes well and monolithic missions
            // poorly, so making the plan stable is a correctness property,
            // not a stylistic preference.
            "generationConfig": { "temperature": 0.0, "maxOutputTokens": 2048 }
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
                Self::validate_success_contracts(&graph)?;
                return Ok(graph);
            }
        }

        if let (Some(start), Some(end)) = (clean.find('{'), clean.rfind('}')) {
            if start <= end {
                let slice = &clean[start..=end];
                if let Ok(mut graph) = serde_json::from_str::<TaskGraph>(slice) {
                    Self::strip_terminal_nodes(&mut graph);
                    if !graph.nodes.is_empty() {
                        Self::validate_success_contracts(&graph)?;
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

                let success_criteria = item
                    .get("success_criteria")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter().filter_map(|x| x.as_str().map(ToString::to_string)).collect()
                    })
                    .unwrap_or_default();

                map.insert(
                    id.clone(),
                    TaskNode {
                        id,
                        deps,
                        required_agent: agent,
                        status: TaskStatus::Pending,
                        objective: obj,
                        success_criteria,
                        execution_state: TaskExecutionState::default(),
                        output_evidence: None,
                        priority: idx + 1,
                    },
                );
            }
            if !map.is_empty() {
                let graph = TaskGraph { nodes: map };
                Self::validate_success_contracts(&graph)?;
                return Ok(graph);
            }
        }

        Ok(Self::create_fallback_graph(directive))
    }

    fn validate_success_contracts(graph: &TaskGraph) -> Result<()> {
        for node in graph.nodes.values() {
            if node.success_criteria.is_empty() {
                return Err(anyhow::anyhow!(
                    "Planner produced real work node '{}' without success criteria.",
                    node.id
                ));
            }

            if node.success_criteria.iter().any(|criterion| criterion.trim().is_empty()) {
                return Err(anyhow::anyhow!(
                    "Planner produced real work node '{}' with an empty success criterion.",
                    node.id
                ));
            }
        }

        Ok(())
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

    fn create_search_memory_graph(directive: &str) -> TaskGraph {
        let mut nodes = HashMap::new();

        nodes.insert(
            "task_1".to_string(),
            TaskNode {
                id: "task_1".to_string(),
                deps: vec![],
                required_agent: "CodeAnalyzer".to_string(),
                status: TaskStatus::Pending,
                objective: directive.to_string(),
                success_criteria: vec!["SEARCH_MEMORY_OK".to_string()],
                execution_state: TaskExecutionState::default(),
                output_evidence: None,
                priority: 1,
            },
        );

        TaskGraph { nodes }
    }

    fn fallback_success_criteria(directive: &str) -> Result<Vec<String>> {
        let lower = directive.to_lowercase();

        // Live host observations require physical command evidence.
        // Keep this aligned with SubgoalCompletionEngine so the DAG planner
        // and Cortex cannot assign contradictory completion contracts.
        if crate::completion_engine::is_host_observation_objective(directive) {
            return Ok(vec!["CMD_OK".to_string()]);
        }

        if lower.contains("command") || lower.contains("execute ") || lower.starts_with("run ") {
            return Ok(vec!["CMD_OK".to_string()]);
        }

        for marker in ["write ", "save ", "export ", "create "] {
            if let Some(pos) = lower.find(marker) {
                let remainder = directive[pos + marker.len()..].trim();
                if let Some(path) = remainder
                    .split_whitespace()
                    .find(|token| token.contains('.') && !token.starts_with("the"))
                {
                    return Ok(vec![format!("WRITE_OK:{}", path.trim_matches('`'))]);
                }
            }
        }

        for marker in ["read ", "inspect "] {
            if let Some(pos) = lower.find(marker) {
                let remainder = directive[pos + marker.len()..].trim();
                if let Some(path) = remainder
                    .split_whitespace()
                    .find(|token| token.contains('/') || token.ends_with(".rs"))
                {
                    return Ok(vec![format!("READ_OK:{}", path.trim_matches('`'))]);
                }
            }
        }

        Err(anyhow::anyhow!(
            "Cannot construct a deterministic fallback success contract for directive."
        ))
    }

    fn create_fallback_graph(directive: &str) -> TaskGraph {
        let mut nodes = HashMap::new();

        let success_criteria = match Self::fallback_success_criteria(directive) {
            Ok(criteria) => criteria,
            Err(error) => {
                tracing::warn!(
                    "Fallback planner cannot construct deterministic success contract: {}",
                    error
                );
                return TaskGraph { nodes };
            }
        };

        nodes.insert(
            "task_1".to_string(),
            TaskNode {
                id: "task_1".to_string(),
                deps: vec![],
                required_agent: "CodeAnalyzer".to_string(),
                status: TaskStatus::Pending,
                objective: directive.to_string(),
                success_criteria,
                execution_state: TaskExecutionState::default(),
                output_evidence: None,
                priority: 1,
            },
        );

        TaskGraph { nodes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_observation_fallback_requires_cmd_ok() {
        let directive = "Perform a read-only investigation and report the current host uptime, filesystem capacity for /, available memory/RAM, and load average. Do not use SearchMemory.";

        assert_eq!(
            PlannerAgent::fallback_success_criteria(directive).unwrap(),
            vec!["CMD_OK".to_string()]
        );
    }

    fn parse(json: &str) -> Result<TaskGraph> {
        PlannerAgent::parse_task_graph(json, "test directive")
    }

    #[test]
    fn explicit_search_memory_directive_creates_native_memory_node() {
        let directive = "Use the SearchMemory tool to recall prior memory about host observations.";

        let graph = PlannerAgent::create_search_memory_graph(directive);

        assert_eq!(graph.nodes.len(), 1);

        let node = &graph.nodes["task_1"];

        assert_eq!(node.required_agent, "CodeAnalyzer");
        assert_eq!(node.objective, directive);
        assert_eq!(node.success_criteria, vec!["SEARCH_MEMORY_OK".to_string()]);

        assert!(!node.success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")));
    }

    #[test]
    fn explicit_search_memory_directive_is_detected_before_model_planning() {
        let source = include_str!("planner.rs");

        let marker = "EXPLICIT NATIVE TOOL CONTRACT:";
        let start =
            source.find(marker).expect("deterministic SearchMemory planner guard must exist");

        let region = &source[start..];

        assert!(region.contains("searchmemory"));
        assert!(region.contains("search memory"));
        assert!(region.contains("search_memory"));
        assert!(region.contains("create_search_memory_graph"));
    }

    #[test]
    fn explicit_success_criteria_are_preserved() {
        let graph = parse(
            r#"{
                "nodes": {
                    "task_1": {
                        "id": "task_1",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "Read router.rs",
                        "success_criteria": ["READ_OK:crates/runtime_core/src/router.rs"]
                    }
                }
            }"#,
        )
        .expect("valid graph should parse");

        assert_eq!(
            graph.nodes["task_1"].success_criteria,
            vec!["READ_OK:crates/runtime_core/src/router.rs"]
        );
    }

    #[test]
    fn missing_success_criteria_are_rejected_for_real_nodes() {
        let result = parse(
            r#"{
                "nodes": {
                    "task_1": {
                        "id": "task_1",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "Read router.rs"
                    }
                }
            }"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn empty_success_criteria_are_rejected_for_real_nodes() {
        let result = parse(
            r#"{
                "nodes": {
                    "task_1": {
                        "id": "task_1",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "Read router.rs",
                        "success_criteria": []
                    }
                }
            }"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn fallback_command_gets_cmd_contract() {
        let criteria =
            PlannerAgent::fallback_success_criteria("execute command `uptime; df -h /; free -m`")
                .expect("command fallback should produce a contract");

        assert_eq!(criteria, vec!["CMD_OK"]);
    }

    #[test]
    fn fallback_write_gets_write_contract() {
        let criteria = PlannerAgent::fallback_success_criteria("write model_routing_analysis.md")
            .expect("write fallback should produce a contract");

        assert_eq!(criteria, vec!["WRITE_OK:model_routing_analysis.md"]);
    }

    #[test]
    fn fallback_read_gets_read_contract() {
        let criteria =
            PlannerAgent::fallback_success_criteria("read crates/runtime_core/src/router.rs")
                .expect("read fallback should produce a contract");

        assert_eq!(criteria, vec!["READ_OK:crates/runtime_core/src/router.rs"]);
    }

    #[test]
    fn fallback_rejects_ambiguous_directive() {
        let result = PlannerAgent::fallback_success_criteria(
            "analyze the architecture and provide recommendations",
        );

        assert!(result.is_err());
    }

    #[test]
    fn terminal_nodes_are_removed_before_contract_validation() {
        let graph = parse(
            r#"{
                "nodes": {
                    "task_1": {
                        "id": "task_1",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "halt"
                    },
                    "task_2": {
                        "id": "task_2",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "Read router.rs",
                        "success_criteria": ["READ_OK:crates/runtime_core/src/router.rs"]
                    }
                }
            }"#,
        )
        .expect("terminal node should be stripped");

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("task_2"));
        assert!(!graph.nodes.contains_key("task_1"));
    }
}
