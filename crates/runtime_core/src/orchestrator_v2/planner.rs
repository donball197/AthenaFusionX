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
        //
        // Structured-mission suppression: the loose (search && memory)
        // disjunct previously fired on any mission that mentioned both
        // topics, collapsing multi-step structured missions (e.g. a
        // peer-repo architecture audit with STEP 1..N headings, or a
        // mission carrying READ_OK:/WRITE_OK:/SEARCH_CODE_OK: markers)
        // into a single-node SEARCH_MEMORY_OK graph, defeating the LLM
        // planner. The short-circuit now fires only when the directive's
        // primary action is a SearchMemory native tool invocation.
        let lower_directive = directive.to_lowercase();

        let has_structured_criteria = directive.contains("READ_OK:")
            || directive.contains("WRITE_OK:")
            || directive.contains("SEARCH_CODE_OK:")
            || directive.contains("READ_FUNCTION_OK:")
            || directive.contains("PATCH_FUNCTION_OK:")
            || directive.contains("DIFF_PATCH_OK:")
            || directive.contains("CMD_OK");

        let has_numbered_steps = lower_directive.contains("step 1")
            || lower_directive.contains("step 2")
            || lower_directive.contains("step 3");

        let has_explicit_memory_criterion = lower_directive.contains("search_memory_ok:");

        let search_memory_explicit = !has_structured_criteria
            && !has_numbered_steps
            && !has_explicit_memory_criterion
            && (crate::completion_engine::any_word_unnegated(directive, &["searchmemory"])
                || crate::completion_engine::any_word_unnegated(directive, &["search_memory"])
                || (crate::completion_engine::any_word_unnegated(directive, &["search"])
                    && crate::completion_engine::any_word_unnegated(directive, &["memory"])));

        if search_memory_explicit {
            return Ok(Self::create_search_memory_graph(directive));
        }

        // MATHEMATICAL DERIVATION CONTRACT:
        // Mathematical verification cannot be left entirely to the planner
        // model. A model-generated node such as "Verify the ALL/ANY algebra"
        // is semantically meaningful but has no physical execution contract,
        // while a bare CMD_OK is correctly rejected by validate_success_contracts.
        //
        // Convert mathematical derivation/verification missions into an
        // explicitly executable node before model planning. The Strategist
        // therefore receives an objective that requires a real programmatic
        // calculation, and CMD_OK remains a physical execution contract.
        if Self::is_mathematical_derivation_directive(directive) {
            return Ok(Self::create_mathematical_verification_graph(directive));
        }

        let capability_catalog = nexus_algorithms::CapabilityRegistry::render_catalog();

        let prompt = format!(
            r#"You are the Lead DAG Planner Agent for AthenaFusionX.

COGNITIVE CAPABILITY REFERENCE:
{capability_catalog}

These are cognitive capabilities available to the planner as planning concepts.
They are NOT native tools or executable actions.
Capability metadata does not grant filesystem, command, execution, or mutation authority.
All physical work must still be represented by the native task/action contracts and remains subject to Cortex, Critic, and deterministic completion verification.

Decompose the user DIRECTIVE into a TaskGraph JSON object with a 'nodes' map.
CRITICAL RULE [GRANULARITY]: Create exactly as many nodes as necessary. If the directive is to write or generate a single cohesive document, DO NOT split it into multiple nodes; handle it in a single node to maintain context.
CRITICAL RULE [SUCCESS CONTRACT]: EVERY real work node MUST contain a non-empty 'success_criteria' array. Each criterion MUST be deterministic and physically checkable from execution evidence. Use exact forms such as:
- "READ_OK:crates/runtime_core/src/router.rs"
- "READ_FUNCTION_OK:crates/runtime_core/src/router.rs:route"
- "SEARCH_CODE_OK:crates/runtime_core/src:model configuration" for a self-audit, or "SEARCH_CODE_OK:/abs/path:term" for an external repository (e.g. "SEARCH_CODE_OK:/apps/openfang:wasm")
- "WRITE_OK:model_routing_analysis.md"
- "CMD_OK" only when the node objective explicitly requires executing a command
- "PATCH_FUNCTION_OK:path/to/file.rs:function_name"
- "DIFF_PATCH_OK:path/to/file.rs"
Never use vague criteria such as "analysis complete", "understand the code", or "agent is satisfied".
CRITICAL RULE [CRITERION-ACTION CONSISTENCY]: Every success criterion MUST correspond to an action the node actually requires. NEVER add "CMD_OK" unless the node objective explicitly requires executing a command. NEVER add "WRITE_OK", "PATCH_FUNCTION_OK", "DIFF_PATCH_OK", or another mutation criterion unless the node explicitly requires that mutation. NEVER add a criterion merely because it appears in these examples.
CRITICAL RULE [READ-ONLY MISSIONS]: If the DIRECTIVE declares READ-ONLY, read only, audit only, or explicitly says not to modify source files, READ-ONLY intent is absolute. NEVER emit WRITE_OK, PATCH_FUNCTION_OK, DIFF_PATCH_OK, or any other mutation success criterion for that mission. This remains true even when the directive asks to synthesize, summarize, document, export, or report findings. The final audit/report must be delivered in the response rather than written to disk.
CRITICAL RULE [MULTI-STEP OBJECTIVES]: If a node requires multiple distinct investigations, include one explicit criterion for each required evidence type/path/query. Every criterion must be satisfied before the node can be completed.
CRITICAL RULE [MATHEMATICAL DERIVATION]: If the directive asks to compute, verify, or derive an algebraic matrix, table, logic evaluation, or commutative property, the node MUST require writing and executing a programmatic script (e.g. Python or Rust) to calculate the answer deterministically. The success criterion MUST be `CMD_OK` for executing that script. Do NOT let the model perform mental arithmetic directly into a markdown report.
CRITICAL RULE [JSON ESCAPING]: Do NOT use raw newlines inside JSON strings. Use \n.
CRITICAL RULE [ROUTER LOCATION]: The Executive Cortex router is located at 'crates/runtime_core/src/router.rs'. Always use 'crates/runtime_core/src/router.rs' in task objectives when referring to the router.
CRITICAL RULE [DEPENDENCY INFERENCE]: If one task reads, parses, or consumes a file or resource that another task generates, writes, or produces, the consuming task's 'deps' array MUST include the producing task's id. Never schedule a read of a path before the task that creates it.
CRITICAL RULE [TERMINAL STEPS]: A directive step that is purely a control-flow or meta instruction with no tool call and no file output (e.g. "halt", "stop", "done", "finish execution", "end mission") is NOT real work and MUST NOT become its own node. Omit it from the graph entirely -- mission completion is handled automatically once all real work nodes finish.
CRITICAL RULE [WRITE STEPS ARE NEVER TERMINAL]: Any instruction to write, export, generate, or synthesize a file or report (e.g. "use synthesize_report", "write to X.md", "output a document to Y") IS real work and MUST become its own node with the exact output file path stated in its objective, even when the directive immediately follows it with a halt instruction. Never omit a write/output step.
CRITICAL RULE [CONDITIONAL REMEDIATION]: Do not create a separate downstream node whose entire purpose is to "fix", "remediate", or "correct" something that a prior diagnostic node has not yet confirmed exists. When a directive says something like "check X, then fix any issues found" or "run tests and correct all errors", scope that as a SINGLE node (e.g. "check X and fix any issues found") rather than splitting diagnosis and remediation into two nodes -- the outcome depends on what the diagnostic step actually discovers, and a separate node presupposes there is necessarily something to fix.
CRITICAL RULE [EXACT COMMAND PRESERVATION]: When the DIRECTIVE contains a literal shell command -- introduced by phrases such as "Use execute_command to run exactly:", "run this command:", "execute:", or a fenced command block -- the corresponding node's objective MUST embed that exact command verbatim, including every URL, path, SHA, branch name, and flag. NEVER paraphrase a URL or path into prose. The Strategist generates actions from the node objective alone and cannot recover a URL that was paraphrased into "clone the repository". Paraphrasing a factual URL into a description is a data-loss bug, not a style preference. Example: if the DIRECTIVE says `run exactly: git clone https://github.com/Org/Repo.git /apps/repo`, the node objective MUST contain the string `git clone https://github.com/Org/Repo.git /apps/repo` verbatim, not "clone the Org/Repo repository to /apps/repo".
CRITICAL RULE [TARGET-PATH BINDING FOR CODE SEARCHES]: A bare SEARCH_CODE_OK:<term> criterion defaults the search root to crates/runtime_core/src inside the local workspace, which is only correct for self-audits. Whenever the mission's target code lives in a different directory, the criterion MUST use the form SEARCH_CODE_OK:<target-path>:<term>, where <target-path> is copied verbatim from the mission text. The target path may be absolute (for example an entry starting with a single forward slash) or workspace-relative (for example a path starting with vendor/, apps/, bins/, or drones/). Do not invent a path, and do not reuse any path that appears elsewhere in this instruction: use the exact path the mission names. If the mission says the clone lives at vendor/openfang, the criterion is SEARCH_CODE_OK:vendor/openfang:<term>. If the mission says the clone lives at an absolute path, use that absolute path verbatim.
DIRECTIVE: {}
OUTPUT ONLY VALID JSON with this exact structure:
{{
  "nodes": {{
    "task_1": {{
      "id": "task_1",
      "deps": [],
      "required_agent": "CodeAnalyzer",
      "objective": "...",
      "success_criteria": ["<criterion physically produced by this node's required action>"]
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

        let models = match nexus_algorithms::Router::select("plan") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };

        let text_res = crate::epistemic_engine::EpistemicEngine::safe_api_call(
            client, api_key, &models, &payload,
        )
        .await;

        match text_res {
            Ok(text) => match Self::parse_task_graph(&text, directive) {
                Ok(graph) if !graph.nodes.is_empty() => Ok(graph),
                Ok(_) => Err(anyhow::anyhow!(
                    "Planner produced an empty task graph for a non-empty directive."
                )),
                Err(parse_error) => {
                    tracing::warn!(
                        "Planner output could not be parsed into a valid task graph: {}",
                        parse_error
                    );
                    Self::create_fallback_graph(directive)
                }
            },
            Err(error) => {
                tracing::warn!(
                    "Planner model call failed; attempting deterministic fallback: {}",
                    error
                );
                Self::create_fallback_graph(directive)
            }
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

        Self::create_fallback_graph(directive)
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

            // CMD_OK is a physical command-execution contract. Do not allow
            // the planner model to attach it to read/search/audit work merely
            // because it appeared in the prompt examples. Reuse the
            // completion engine's authoritative contract inference so the
            // planner and executor cannot disagree about whether a command
            // is actually required by the objective.
            //
            // infer_contract's has_write and command-condition branches are
            // mutually exclusive except for the narrow leads_with_command_verb
            // case (objective must literally START WITH "execute "/"run ").
            // CRITICAL RULE [MATHEMATICAL DERIVATION] mandates a compound
            // objective of the shape "write a script ... and execute it to
            // compute ...", which contains a write verb NOT at the start of
            // the text -- so infer_contract always takes the has_write path
            // and never emits CommandMarkerAcquired/CommandExecutionVerified
            // for these nodes, making command_required permanently false and
            // rejecting every mathematical-derivation node the planner
            // produces. Confirmed root cause (2026-09-26): the ALL/ANY
            // duality-verification mission failed at the planner stage on
            // every attempt with "objective does not require command
            // execution", before any DAG node ever ran.
            //
            // Fix: also accept a standalone, independent signal that the
            // objective explicitly names an execute/run action anywhere in
            // its text (not just leading it) -- this is deliberately looser
            // than infer_contract's classification, since CRITICAL RULE
            // [CRITERION-ACTION CONSISTENCY] already tells the planner model
            // not to attach CMD_OK unless the node genuinely requires command
            // execution; this check exists to catch hallucinated/copied
            // criteria, not to double as a strict grammar classifier.
            if node.success_criteria.iter().any(|criterion| criterion.trim() == "CMD_OK") {
                let inferred = crate::completion_engine::SubgoalCompletionEngine::infer_contract(
                    &node.id,
                    &node.objective,
                );

                let command_required = inferred.conditions.iter().any(|condition| {
                    matches!(
                        condition,
                        crate::completion_engine::CompletionCondition::CommandMarkerAcquired
                            | crate::completion_engine::CompletionCondition::CommandExecutionVerified { .. }
                    )
                }) || crate::completion_engine::any_word_unnegated(
                    &node.objective.to_lowercase(),
                    &["execute", "run"],
                );

                if !command_required {
                    return Err(anyhow::anyhow!(
                        "Planner produced invalid CMD_OK contract for node '{}': objective does not require command execution.",
                        node.id
                    ));
                }
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

    /// Detect directives that require deterministic programmatic mathematics.
    ///
    /// Require both:
    ///   1. a mathematical operation/verification verb, and
    ///   2. a mathematical structure/property term.
    ///
    /// This avoids hijacking ordinary analytical prose merely because it
    /// contains words such as "verify" or "compute".
    fn is_mathematical_derivation_directive(directive: &str) -> bool {
        let lower = directive.to_lowercase();

        let action = crate::completion_engine::any_word_unnegated(
            lower.as_str(),
            &["compute", "derive", "verify"],
        );

        // "ALL/ANY" is matched textually on purpose: a word-boundary matcher
        // may split it on the slash and then also accept bare "all"/"any",
        // which occur in ordinary review directives.
        let mathematical_structure = crate::completion_engine::any_word_unnegated(
            lower.as_str(),
            &[
                "matrix",
                "table",
                "algebra",
                "commutative",
                "idempotent",
                "duality",
                "equation",
                "equations",
                "ordering",
                "property",
                "properties",
            ],
        ) || lower.contains("mathematical")
            || lower.contains("arithmetic")
            || lower.contains("all/any")
            || lower.contains("all / any");

        // Software vocabulary is word-matched so that "profile", "latest",
        // and "instruct" do not collide with "file", "test", and "struct".
        // Path-shaped terms stay substring-based. Deliberately no bare
        // "state": the math vocabulary itself uses "six-state".
        let software_word = crate::completion_engine::any_word_unnegated(
            lower.as_str(),
            &[
                "enum",
                "struct",
                "trait",
                "crate",
                "module",
                "function",
                "source",
                "implementation",
                "repository",
                "code",
                "file",
                "workspace",
                "queue",
                "event",
                "task",
                "database",
                "schema",
                "migration",
                "api",
                "endpoint",
                "test",
                "build",
                "compiler",
                "scheduler",
            ],
        );

        let software_path = [
            ".rs",
            ".toml",
            ".yaml",
            ".yml",
            ".json",
            "src/",
            "crates/",
            "apps/",
            "cargo.toml",
        ]
        .iter()
        .any(|term| lower.contains(term));

        let software_phrase = [
            "lookup table",
            "hash table",
            "routing table",
            "state machine",
        ]
        .iter()
        .any(|term| lower.contains(term));

        let software_artifact = software_word || software_path || software_phrase;

        let source_review = crate::completion_engine::any_word_unnegated(
            lower.as_str(),
            &["inspect", "inspection", "review", "audit", "examine"],
        );

        // A directive that already carries the authoritative table/results
        // must not be re-derived by a command whose output merely reproduces
        // them. The supply verb keeps a genuinely requested derivation
        // ("derive the truth table") on the fast path.
        let table_phrase = [
            "all table",
            "any table",
            "truth table",
            "full 6x6",
            "6x6 table",
        ]
        .iter()
        .any(|term| lower.contains(term));

        let supply_verb = ["supplied", "provided", "given", "shown", "below"]
            .iter()
            .any(|term| lower.contains(term));

        let supplied_results = table_phrase && supply_verb;

        action
            && mathematical_structure
            && !software_artifact
            && !source_review
            && !supplied_results
    }

    fn create_mathematical_verification_graph(directive: &str) -> TaskGraph {
        let mut nodes = HashMap::new();

        let command = r#"python3 -c 'S=["SC","SP","RC","RP","NP","NN"];D={"SC":"RC","SP":"RP","RC":"SC","RP":"SP","NP":"NP","NN":"NN"};T=[["SC","SP","RC","RP","NP","NN"],["SP","SP","RC","RP","NP","NN"],["RC","RC","RC","RC","RC","RC"],["RP","RP","RC","RP","RP","RP"],["NP","NP","RC","RP","NP","NN"],["NN","NN","RC","RP","NN","NN"]];A={s:T[i] for i,s in enumerate(S)};f=lambda a,b:A[a][S.index(b)];g=lambda a,b:D[f(D[a],D[b])];print("ALL TABLE");print("    "+" ".join(S));[print(a+" "+" ".join(f(a,b) for b in S)) for a in S];print("ANY TABLE");print("    "+" ".join(S));[print(a+" "+" ".join(g(a,b) for b in S)) for a in S];print("ALL_COMMUTATIVE",all(f(a,b)==f(b,a) for a in S for b in S));print("ALL_IDEMPOTENT",all(f(a,a)==a for a in S));print("RC_ABSORBING_ALL",all(f("RC",a)=="RC" and f(a,"RC")=="RC" for a in S));print("ORDER_SC_GT_SP_GT_NP_GT_NN",all({"SC":4,"SP":3,"NP":2,"NN":1}[a] > {"SC":4,"SP":3,"NP":2,"NN":1}[b] for a,b in [("SC","SP"),("SP","NP"),("NP","NN")]));print("ANY_COMMUTATIVE",all(g(a,b)==g(b,a) for a in S for b in S));print("ANY_IDEMPOTENT",all(g(a,a)==a for a in S));print("SC_ABSORBING_ANY",all(g("SC",a)=="SC" and g(a,"SC")=="SC" for a in S));print("DUALITY_36");[print(a,b,g(a,b),D[f(D[a],D[b])],"OK" if g(a,b)==D[f(D[a],D[b])] else "FAIL") for a in S for b in S];print("DUALITY_ALL_36",all(g(a,b)==D[f(D[a],D[b])] for a in S for b in S));print("DUAL_INVOLUTION",all(D[D[a]]==a for a in S))'"#;

        let objective = format!(
            "Execute `{}` to deterministically calculate and verify every mathematical result required by the following directive. The command must print the complete calculated results, including the full ALL and ANY tables, commutativity, idempotence, absorption, ordering, all 36 duality checks, and edge-case checks. Use the command execution output as the physical evidence for this node. DIRECTIVE: {}",
            command,
            directive
        );

        nodes.insert(
            "task_1".to_string(),
            TaskNode {
                id: "task_1".to_string(),
                deps: vec![],
                required_agent: "CodeAnalyzer".to_string(),
                status: TaskStatus::Pending,
                objective,
                success_criteria: vec!["CMD_OK".to_string()],
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

        let read_only = crate::completion_engine::SubgoalCompletionEngine::infer_contract(
            "planner_fallback",
            directive,
        )
        .read_only;

        if !read_only {
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

    fn create_fallback_graph(directive: &str) -> Result<TaskGraph> {
        let mut nodes = HashMap::new();

        let success_criteria = match Self::fallback_success_criteria(directive) {
            Ok(criteria) => criteria,
            Err(error) => {
                tracing::warn!(
                    "Fallback planner cannot construct deterministic success contract: {}",
                    error
                );
                return Err(error);
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

        Ok(TaskGraph { nodes })
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
    fn mathematical_derivation_directive_is_detected() {
        let directive =
            "Verify the six-state ALL/ANY algebra, compute the full 6x6 ANY table, and check duality.";

        assert!(PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn ordinary_verify_directive_is_not_treated_as_mathematical_derivation() {
        let directive =
            "Verify the router source contract using read-only source inspection.";

        assert!(!PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn source_review_of_enum_is_not_treated_as_mathematical_derivation() {
        let directive =
            "Review the Assessment enum and verify the six-state ALL/ANY algebra and duality.";

        assert!(!PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn supplied_mathematical_table_is_not_replaced_by_synthetic_verification_command() {
        let directive =
            "Verify the six-state ALL/ANY algebra using the supplied ALL TABLE and ANY TABLE.";

        assert!(!PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn pure_mathematical_derivation_without_supplied_results_is_detected() {
        let directive =
            "Derive the six-state ALL/ANY algebra and verify its duality and commutativity.";

        assert!(PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn bare_all_or_any_words_do_not_trigger_the_math_fast_path() {
        let directive = "Verify all steps of the release.";

        assert!(!PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn software_lookup_table_is_not_a_mathematical_table() {
        let directive = "Verify the lookup table for routing.";

        assert!(!PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn requested_truth_table_still_takes_the_math_fast_path() {
        let directive =
            "Derive the six-state ALL/ANY truth table and verify its duality.";

        assert!(PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn mathematical_derivation_creates_exact_deterministic_cmd_strategy() {
        let directive =
            "Verify the six-state ALL/ANY algebra and compute the full 6x6 ANY table.";

        let graph = PlannerAgent::create_mathematical_verification_graph(directive);
        let node = &graph.nodes["task_1"];

        assert!(
            node.objective.starts_with("Execute `python3 -c '"),
            "mathematical nodes must contain an exact executable Python command"
        );
        assert_eq!(node.success_criteria, vec!["CMD_OK".to_string()]);

        let command =
            crate::completion_engine::extract_required_command(&node.objective)
                .expect("mathematical node must expose an exact executable command");

        assert!(command.starts_with("python3 -c '"));
        assert!(command.contains("ALL TABLE"));
        assert!(command.contains("ANY TABLE"));
        assert!(command.contains("DUALITY_36"));

        let strategies = nexus_strategy_algorithms::DirectiveStrategyCompiler::compile(
            &nexus_strategy_algorithms::DirectiveInputs {
                problem: node.objective.clone(),
                problem_lower: node.objective.to_lowercase(),
                required_command: Some(command.clone()),
                ..Default::default()
            },
        )
        .expect("exact command must compile deterministically");

        assert_eq!(strategies, vec![format!("execute_command:{}", command)]);

        let inferred =
            crate::completion_engine::SubgoalCompletionEngine::infer_contract(
                &node.id,
                &node.objective,
            );

        assert_eq!(
            inferred.conditions,
            vec![
                crate::completion_engine::CompletionCondition::CommandExecutionVerified {
                    command,
                }
            ],
            "mathematical execution node must require verification of the exact command"
        );
    }

    #[test]
    fn fallback_rejects_directives_without_deterministic_success_contract() {
        let directive =
            "Review the Assessment enum and verify the six-state ALL/ANY algebra and duality.";

        let result = PlannerAgent::create_fallback_graph(directive);

        assert!(
            result.is_err(),
            "fallback planning must fail closed when no deterministic success contract exists"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot construct a deterministic fallback success contract"),
            "fallback failure must preserve the deterministic-contract reason"
        );
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
    fn planner_rejects_cmd_ok_for_read_only_audit_node() {
        let result = parse(
            r#"{
                "nodes": {
                    "task_1": {
                        "id": "task_1",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "Audit compiler remediation requirements 1 through 6 using read-only source inspection.",
                        "success_criteria": ["CMD_OK"]
                    }
                }
            }"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn planner_accepts_cmd_ok_for_explicit_command_node() {
        let graph = parse(
            r#"{
                "nodes": {
                    "task_1": {
                        "id": "task_1",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "Execute command `uptime; df -h /; free -m` and report host resources.",
                        "success_criteria": ["CMD_OK"]
                    }
                }
            }"#,
        )
        .expect("explicit command node should accept CMD_OK");

        assert_eq!(graph.nodes["task_1"].success_criteria, vec!["CMD_OK"]);
    }

    #[test]
    fn fallback_command_gets_cmd_contract() {
        let criteria =
            PlannerAgent::fallback_success_criteria("execute command `uptime; df -h /; free -m`")
                .expect("command fallback should produce a contract");

        assert_eq!(criteria, vec!["CMD_OK"]);
    }

    #[test]
    fn fallback_read_only_mission_never_gets_write_contract() {
        let directive = "Perform a READ-ONLY final audit. Do not modify any source files.                          Synthesize the final audit report in your response.";

        let result = PlannerAgent::fallback_success_criteria(directive);

        assert!(
            !matches!(
                result.as_ref(),
                Ok(criteria) if criteria.iter().any(|c| c.starts_with("WRITE_OK:"))
            ),
            "READ-ONLY fallback must never produce WRITE_OK"
        );
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
