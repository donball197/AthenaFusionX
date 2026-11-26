use crate::orchestrator_v2::task_graph::{TaskExecutionState, TaskGraph, TaskNode, TaskStatus};
use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;

pub struct PlannerAgent;

/// Conservative READ-ONLY detector. Matches the phrasing the cortex's
/// contract inference uses, so planner rewrite and cortex inference
/// agree on which directives are audits.
fn directive_is_read_only(directive: &str) -> bool {
    let lower = directive.to_lowercase();
    lower.contains("read-only")
        || lower.contains("read only")
        || lower.contains("readonly")
        || lower.contains("do not modify")
        || lower.contains("don't modify")
        || lower.contains("audit only")
        || lower.contains("inspection only")
}

/// For READ-ONLY missions, rewrite a node's SEARCH_CODE_OK: criteria to
/// SEARCH_CHECKED_OK: when the node carries no mutation criterion. A
/// node that also patches/writes is left strict: its search feeds the
/// mutation, and a no-match really does block the mission.
/// True when a criterion represents a mutation of ANY kind, including
/// a report/artifact write. Used by the READ-ONLY rewrite, where the
/// contract explicitly forbids `WRITE_OK:`-family criteria entirely and
/// delivers the report in the response rather than to disk. For the
/// audit rewrite, use `criterion_is_source_mutation` instead: audit
/// missions intentionally allow report writes while still gating on
/// source changes.
fn criterion_is_any_mutation(criterion: &str) -> bool {
    let t = criterion.trim();
    t.starts_with("WRITE_OK:")
        || t.starts_with("PATCH_FUNCTION_OK:")
        || t.starts_with("DIFF_PATCH_OK:")
        || t.starts_with("REPLACE_LINES_OK:")
}

fn apply_read_only_search_rewrite(graph: &mut TaskGraph, directive: &str) {
    if !directive_is_read_only(directive) {
        return;
    }
    // Positive-match guard: a read-only directive that says "find all",
    // "locate every", "must find", etc. must not be weakened to
    // checked-search. The criterion is a mission obligation, not an
    // audit nicety.
    if directive_requires_positive_match(directive) {
        return;
    }
    // Graph-wide ANY-mutation guard: READ-ONLY missions forbid every
    // `WRITE_OK:`-family criterion -- report artifacts included. If the
    // graph carries any such criterion, the graph already violates the
    // READ-ONLY contract; refusing to rewrite is the safe response
    // rather than layering a second policy change on a malformed graph.
    //
    // This uses `criterion_is_any_mutation`, NOT
    // `criterion_is_source_mutation`: READ-ONLY rejects a
    // `WRITE_OK:report.md` even though that write does not touch
    // source, and the audit rewrite's source-only classification would
    // silently weaken that contract. The two policies are intentionally
    // distinct.
    let graph_has_any_mutation = graph
        .nodes
        .values()
        .any(|n| n.success_criteria.iter().any(|c| criterion_is_any_mutation(c)));
    if graph_has_any_mutation {
        return;
    }
    for node in graph.nodes.values_mut() {
        for criterion in node.success_criteria.iter_mut() {
            if criterion.starts_with("SEARCH_CODE_OK:") {
                if let Some(target) = criterion.strip_prefix("SEARCH_CODE_OK:") {
                    if !checked_term_is_concrete(target) {
                        continue;
                    }
                }
                *criterion = criterion.replacen("SEARCH_CODE_OK:", "SEARCH_CHECKED_OK:", 1);
            }
        }
    }
}

/// True when the directive reads as an audit / survey / inventory
/// objective: "look and report", not "find and change". Deliberately
/// narrow -- `report` and `analysis` alone are NOT audit signals because
/// "write a report from this data" and "analyze this function" do not
/// imply that a search result is expected to be empty.
fn directive_has_audit_intent(directive: &str) -> bool {
    let lower = directive.to_lowercase();
    lower.contains("audit") || lower.contains("survey") || lower.contains("inventory")
}

/// True when the directive contains an explicit "must find" phrase that
/// would make a no-match search a mission failure, not a valid finding.
/// These phrases override the audit rewrite even when the audit intent is
/// otherwise present, so "Audit the code to locate every deprecated_fn"
/// keeps strict search semantics.
fn directive_requires_positive_match(directive: &str) -> bool {
    let lower = directive.to_lowercase();
    [
        "must find",
        "must locate",
        "must be found",
        "locate every",
        "locate all",
        "find every",
        "find all",
        "find where",
        "all occurrences",
        "all usages of",
        "every occurrence",
        "every usage of",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// True when a single criterion represents a source-code mutation (as
/// opposed to writing a new report/artifact). `WRITE_OK:` gates the
/// audit rewrite only when the target path is a `.rs` source file, so
/// `WRITE_OK:full_system_audit.md` (a report) does NOT gate while
/// `WRITE_OK:crates/foo/src/new.rs` (a source addition/modification) does.
fn criterion_is_source_mutation(criterion: &str) -> bool {
    let t = criterion.trim();
    if t.starts_with("PATCH_FUNCTION_OK:")
        || t.starts_with("DIFF_PATCH_OK:")
        || t.starts_with("REPLACE_LINES_OK:")
    {
        return true;
    }
    if let Some(path) = t.strip_prefix("WRITE_OK:") {
        return path.trim().ends_with(".rs");
    }
    false
}

/// Rewrite every `SEARCH_CODE_OK:` criterion in an audit-class graph to
/// `SEARCH_CHECKED_OK:`, so that a documented no-match counts as a valid
/// audit finding rather than a failure.
///
/// Policy (deliberately conservative, documented here on purpose):
/// * Fires only when the directive reads as an audit ("audit", "survey",
///   "inventory") AND contains no explicit "must find" phrase.
/// * If ANY node in the graph carries a source-mutation criterion, the
///   WHOLE graph keeps strict search semantics. A graph-wide gate is
///   simpler and safer than per-node: two search nodes that would
///   otherwise be audit-checked could each be consumed by a downstream
///   source-mutation node whose dependency the criteria cannot express,
///   so per-node gating would weaken the search in a way the verifier
///   could not detect.
/// * Report/artifact writes (`WRITE_OK:<non-.rs-path>`) do NOT gate the
///   rewrite: writing a new `.md`/`.txt`/`.json`/`.html` audit report
///   does not require any search to have found anything.
/// * The evidence verifier is NOT relaxed: `SEARCH_CODE_OK:` still
///   requires a positive match, and `SEARCH_CHECKED_OK:` is the only
///   criterion that accepts a documented no-match.
fn apply_audit_search_rewrite(graph: &mut TaskGraph, directive: &str) {
    if !directive_has_audit_intent(directive) {
        return;
    }
    if directive_requires_positive_match(directive) {
        return;
    }
    let graph_has_source_mutation = graph
        .nodes
        .values()
        .any(|n| n.success_criteria.iter().any(|c| criterion_is_source_mutation(c)));
    if graph_has_source_mutation {
        return;
    }
    for node in graph.nodes.values_mut() {
        for criterion in node.success_criteria.iter_mut() {
            if criterion.starts_with("SEARCH_CODE_OK:") {
                if let Some(target) = criterion.strip_prefix("SEARCH_CODE_OK:") {
                    if !checked_term_is_concrete(target) {
                        continue;
                    }
                }
                *criterion = criterion.replacen("SEARCH_CODE_OK:", "SEARCH_CHECKED_OK:", 1);
            }
        }
    }
}

/// Files the directive names that exist in the workspace and are small enough to
/// read whole. Relative paths only; reports, logs and lockfiles are skipped.
fn named_workspace_files(directive: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in directive.split_whitespace() {
        let t = raw.trim_matches(|c: char| {
            !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
        });
        if t.is_empty()
            || !t.contains('.')
            || t.starts_with('/')
            || t.contains("..")
            || t.ends_with(".md")
            || t.ends_with(".log")
            || t.ends_with(".lock")
        {
            continue;
        }
        let path = std::path::Path::new(t);
        let small =
            std::fs::metadata(path).map(|m| m.is_file() && m.len() <= 65_536).unwrap_or(false);
        if !small {
            continue;
        }
        if !out.iter().any(|e| e == t) {
            out.push(t.to_string());
        }
        if out.len() >= 6 {
            break;
        }
    }
    out
}

/// Audit missions that write a report file must carry an inspection criterion.
/// The decomposer LLM is nondeterministic: the same prompt once produced
/// per-directory search criteria and once produced none, which let a template
/// report be written before any evidence existed. Floor: read the files the
/// directive names (see `named_workspace_files`) before the report is written.
fn apply_audit_recon_floor(graph: &mut TaskGraph, directive: &str) {
    if !directive_has_audit_intent(directive) {
        return;
    }
    let named = named_workspace_files(directive);
    if named.is_empty() {
        return;
    }
    for node in graph.nodes.values_mut() {
        let writes_report = node.success_criteria.iter().any(|c| {
            let t = c.trim();
            t.starts_with("WRITE_OK:") && !criterion_is_source_mutation(t)
        });
        let has_inspection = node.success_criteria.iter().any(|c| criterion_is_inspection_only(c));
        if !writes_report || has_inspection {
            continue;
        }
        for file in &named {
            node.success_criteria.push(format!("READ_OK:{}", file));
        }
    }
}

/// True when the directive says the report is delivered in the response
/// (not written to disk).
fn directive_delivers_report_inline(directive: &str) -> bool {
    let l = directive.to_lowercase();
    let has_report_word = l.contains("report") || l.contains("findings");
    if !has_report_word {
        return false;
    }
    // Explicit inline markers — always accept.
    if l.contains("inline")
        || l.contains("in the response")
        || l.contains("in your response")
        || l.contains("in your final response")
    {
        return true;
    }
    // Implicit: "return a/the/your ... report" — a direct verb applying to
    // the report as the deliverable. Covers "Return a structured audit
    // report containing..." which was not caught by the previous version,
    // and which caused run a4530759 to complete without any report.
    let has_return_verb = l.contains("return a")
        || l.contains("return the")
        || l.contains("return your")
        || l.contains("deliver a")
        || l.contains("deliver the")
        || l.contains("deliver your")
        || l.contains("produce a")
        || l.contains("produce the")
        || l.contains("produce your");
    if !has_return_verb {
        return false;
    }
    // File-write directives are NOT inline. If the same directive
    // specifies a destination path or "to disk" / "to a file", the write
    // is the deliverable, and the write path handles it separately.
    let writes_to_file = l.contains(".md")
        || l.contains(".txt")
        || l.contains(".json")
        || l.contains(".yaml")
        || l.contains(".yml")
        || l.contains("to disk")
        || l.contains("to a file")
        || l.contains("to the file")
        || l.contains("write a file");
    !writes_to_file
}

fn criterion_is_inspection_only(criterion: &str) -> bool {
    let t = criterion.trim();
    t.starts_with("READ_OK:")
        || t.starts_with("READ_FUNCTION_OK:")
        || t.starts_with("SEARCH_CODE_OK:")
        || t.starts_with("SEARCH_CHECKED_OK:")
}

/// A read-only audit that must deliver its report in the response needs a
/// deliverable criterion, otherwise the node completes on searches/reads
/// alone and the report is never required. Adds ANALYSIS_OK to the sink node
/// when (and only when) that node is pure inspection work and nothing in the
/// graph already carries ANALYSIS_OK or a mutation criterion.
/// Extract write-deliverable paths from the directive text.
///
/// Consumes a fixed verb allowlist and returns the first non-absolute
/// path token after each verb that has a reportable extension. Longest
/// verb wins ("write the result to X.md" matches before "write ").
///
/// Deliberately conservative:
///   * only `.md`, `.txt`, `.json`, `.yaml`, `.yml` extensions
///   * only relative or bare paths (rejects absolute and `..`)
///   * no wildcards, no globs, no directory names
pub(crate) fn extract_write_targets_from_directive(directive: &str) -> Vec<String> {
    const VERBS: &[&str] = &[
        "write the result to ",
        "write the findings to ",
        "write findings to ",
        "write the report to ",
        "write to ",
        "write ",
        "save the result to ",
        "save the findings to ",
        "save to ",
        "save ",
        "export to ",
        "export ",
        "output a report to ",
        "output a document to ",
        "output a file to ",
        "output to ",
        "produce a report named ",
        "produce a report at ",
        "synthesize the report to ",
        "synthesize ",
        "deliver a report to ",
        "deliver ",
        "create the file ",
        "create file ",
        "create ",
    ];

    let lower = directive.to_lowercase();
    let mut sorted: Vec<&str> = VERBS.iter().copied().collect();
    sorted.sort_by_key(|v| std::cmp::Reverse(v.len()));

    let mut out: Vec<String> = Vec::new();
    for verb in sorted {
        let mut search_from = 0;
        while let Some(pos) = lower[search_from..].find(verb) {
            let abs = search_from + pos + verb.len();
            let rest = &directive[abs..];
            let raw_tok = rest.split_whitespace().next().unwrap_or("");

            // Directives are prose, not raw identifiers. Trim SENTENCE
            // punctuation from both ends, keep interior dots so
            // `file.v1.2.md` stays intact. This is what fixes the
            // `mission_04_worker_recovery.md.` (trailing period) case.
            let t = raw_tok
                .trim_end_matches(|c: char| {
                    matches!(
                        c,
                        '.' | '!' | '?' | ',' | ';' | ')' | ':' | '"' | '\'' | '`'
                    )
                })
                .trim_start_matches(|c: char| matches!(c, '(' | '"' | '\'' | '`'));

            let is_report = t.ends_with(".md")
                || t.ends_with(".txt")
                || t.ends_with(".json")
                || t.ends_with(".yaml")
                || t.ends_with(".yml");
            let is_ok_path =
                !t.is_empty() && !t.starts_with('/') && !t.contains("..");
            if is_report && is_ok_path && !out.iter().any(|e| e == t) {
                out.push(t.to_string());
            }
            search_from = abs;
        }
    }
    out
}

/// Ensure every write deliverable named in the directive has a
/// corresponding `WRITE_OK:` criterion on some node.
///
/// If the graph already covers a target, no change. If not, the sink
/// node (nothing depends on it, most criteria) receives the criterion.
/// If there are no sinks (all nodes have dependents -- a cycle), do
/// nothing and let validation fail elsewhere.
///
/// This is the load-bearing fix for planner under-decomposition when
/// a multi-part directive combines investigation with a written
/// deliverable: the model frequently keeps the investigation and
/// silently drops the write.
fn apply_write_deliverable_criterion(graph: &mut TaskGraph, directive: &str) {
    let targets = extract_write_targets_from_directive(directive);
    if targets.is_empty() {
        return;
    }
    for target in targets {
        let covered = graph.nodes.values().any(|n| {
            n.success_criteria.iter().any(|c| {
                let t = c.trim();
                t.starts_with("WRITE_OK:") && t.ends_with(&target)
            })
        });
        if covered {
            continue;
        }
        let mut sinks: Vec<String> = graph
            .nodes
            .iter()
            .filter(|(id, _)| {
                !graph
                    .nodes
                    .values()
                    .any(|n| n.deps.iter().any(|d| d == *id))
            })
            .map(|(id, _)| id.clone())
            .collect();
        sinks.sort_by_key(|id| std::cmp::Reverse(graph.nodes[id].success_criteria.len()));
        if let Some(id) = sinks.first() {
            if let Some(n) = graph.nodes.get_mut(id) {
                n.success_criteria.push(format!("WRITE_OK:{}", target));
                tracing::warn!(
                    "PLANNER_DELIVERABLE_ENFORCEMENT: added WRITE_OK:{} to node '{}' \
                     (directive names this deliverable but graph had no matching criterion)",
                    target, id
                );
            }
        }
    }
}

fn apply_report_deliverable_criterion(graph: &mut TaskGraph, directive: &str) {
    if !(directive_is_read_only(directive) || directive_has_audit_intent(directive)) {
        return;
    }
    if !directive_delivers_report_inline(directive) {
        return;
    }
    if graph.nodes.values().any(|n| {
        n.success_criteria.iter().any(|c| c.trim() == "ANALYSIS_OK" || criterion_is_any_mutation(c))
    }) {
        return;
    }
    let depended: std::collections::HashSet<String> =
        graph.nodes.values().flat_map(|n| n.deps.iter().cloned()).collect();
    let mut sinks: Vec<String> = graph
        .nodes
        .iter()
        .filter(|(id, n)| {
            !depended.contains(id.as_str())
                && !n.success_criteria.is_empty()
                && n.success_criteria.iter().all(|c| criterion_is_inspection_only(c))
        })
        .map(|(id, _)| id.clone())
        .collect();
    sinks.sort();
    if let Some(id) = sinks.pop() {
        if let Some(n) = graph.nodes.get_mut(&id) {
            n.success_criteria.push("ANALYSIS_OK".to_string());
        }
    }
}

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
            || directive.contains("SEARCH_CHECKED_OK:")
            || directive.contains("READ_FUNCTION_OK:")
            || directive.contains("PATCH_FUNCTION_OK:")
            || directive.contains("DIFF_PATCH_OK:")
            || directive.contains("CMD_OK");

        let has_numbered_steps = lower_directive.contains("step 1")
            || lower_directive.contains("step 2")
            || lower_directive.contains("using only")
            || lower_directive.contains("only the execute_command")
            || lower_directive.contains("step 3");

        let has_explicit_memory_criterion = lower_directive.contains("search_memory_ok:");

        let search_memory_explicit = !has_structured_criteria
            && !has_numbered_steps
            && !has_explicit_memory_criterion
            && (crate::completion_engine::any_word_unnegated(directive, &["searchmemory"])
                || crate::completion_engine::any_word_unnegated(directive, &["search_memory"])
                || (crate::completion_engine::any_word_unnegated(directive, &["search"])
                    && crate::completion_engine::any_word_unnegated(directive, &["memory"])
                    && is_short_plain_directive(directive)));

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
CRITICAL RULE [GRANULARITY]: Create exactly as many nodes as necessary. If the directive asks to audit/investigate the repository or write/generate a single cohesive report (e.g. 'mission_06_arm64_efficiency.md'), keep it as ONE node so the writer retains all read/search/command evidence in context.
CRITICAL RULE [SUCCESS CONTRACT]: EVERY real work node MUST contain a non-empty 'success_criteria' array. Each criterion MUST be deterministic and physically checkable from execution evidence. Use exact forms such as:
- "READ_OK:crates/runtime_core/src/router.rs"
- "READ_FUNCTION_OK:crates/runtime_core/src/router.rs:route"
- "SEARCH_CODE_OK:crates/runtime_core/src:model configuration" for a self-audit, or "SEARCH_CODE_OK:/abs/path:term" for an external repository (e.g. "SEARCH_CODE_OK:/apps/external_repo:wasm"). For audit-style searches where a no-match is a valid finding, use the SEARCH_CHECKED_OK: form of either criterion (see [AUDIT SEARCH USES CHECKED FORM])
- "WRITE_OK:model_routing_analysis.md"
- "CMD_OK" only when the node objective explicitly requires executing a command
- "PATCH_FUNCTION_OK:path/to/file.rs:function_name"
- "DIFF_PATCH_OK:path/to/file.rs"
- "ANALYSIS_OK" ONLY when the directive supplies the code or data to analyze inline, names no file to read or search, and asks for a written answer (audit, review, explanation). The answer must be delivered via halt.
Never use vague criteria such as "analysis complete", "understand the code", or "agent is satisfied".
CRITICAL RULE [DELIVERABLE PRESERVATION]: If the directive asks to read or search something AND then report, identify, audit or explain, keep that as ONE node whose objective states the full reporting requirement and required output format verbatim, with criteria ["READ_OK:<path>", "ANALYSIS_OK"]. Never reduce such a node to only the read step.
CRITICAL RULE [INLINE ANALYSIS]: When the directive contains the code to audit inline and does not name a file to read, use exactly ["ANALYSIS_OK"]. Never add READ_OK or SEARCH_CODE_OK for files the directive does not name.
CRITICAL RULE [CRITERION-ACTION CONSISTENCY]: Every success criterion MUST correspond to an action the node actually requires. NEVER add "CMD_OK" unless the node objective explicitly requires executing a command. NEVER add "WRITE_OK", "PATCH_FUNCTION_OK", "DIFF_PATCH_OK", or another mutation criterion unless the node explicitly requires that mutation. NEVER add a criterion merely because it appears in these examples.
CRITICAL RULE [READ-ONLY MISSIONS]: If the DIRECTIVE declares READ-ONLY, read only, audit only, or explicitly says not to modify source files, READ-ONLY intent is absolute. NEVER emit WRITE_OK, PATCH_FUNCTION_OK, DIFF_PATCH_OK, or any other mutation success criterion for that mission. This remains true even when the directive asks to synthesize, summarize, document, export, or report findings. The final audit/report must be delivered in the response rather than written to disk.
CRITICAL RULE [MULTI-STEP OBJECTIVES]: If a node requires multiple distinct investigations, include one explicit criterion for each required evidence type/path/query. Every criterion must be satisfied before the node can be completed.
CRITICAL RULE [MATHEMATICAL DERIVATION]: If the directive asks to compute, verify, or derive an algebraic matrix, table, logic evaluation, or commutative property, the node MUST require writing and executing a programmatic script (e.g. Python or Rust) to calculate the answer deterministically. The success criterion MUST be `CMD_OK` for executing that script. Do NOT let the model perform mental arithmetic directly into a markdown report.
CRITICAL RULE [JSON ESCAPING]: Do NOT use raw newlines inside JSON strings. Use \n.
CRITICAL RULE [ROUTER LOCATION]: Only refer to 'crates/runtime_core/src/router.rs' when the directive is actually about directive routing. Do NOT force 'router.rs' for mission-completion, verifier, executor, or worker-lifecycle audits; inspect the relevant runtime_core modules instead.
CRITICAL RULE [DEPENDENCY INFERENCE]: If one task reads, parses, or consumes a file or resource that another task generates, writes, or produces, the consuming task's 'deps' array MUST include the producing task's id. Never schedule a read of a path before the task that creates it.
CRITICAL RULE [TERMINAL STEPS]: A directive step that is purely a control-flow or meta instruction with no tool call and no file output (e.g. "halt", "stop", "done", "finish execution", "end mission") is NOT real work and MUST NOT become its own node. Omit it from the graph entirely -- mission completion is handled automatically once all real work nodes finish.
CRITICAL RULE [WRITE STEPS ARE NEVER TERMINAL]: Any instruction to write, export, generate, or synthesize a file or report (e.g. "use synthesize_report", "write to X.md", "output a document to Y") IS real work and MUST become its own node with the exact output file path stated in its objective, even when the directive immediately follows it with a halt instruction. Never omit a write/output step.
CRITICAL RULE [CONDITIONAL REMEDIATION]: Do not create a separate downstream node whose entire purpose is to "fix", "remediate", or "correct" something that a prior diagnostic node has not yet confirmed exists. When a directive says something like "check X, then fix any issues found" or "run tests and correct all errors", scope that as a SINGLE node (e.g. "check X and fix any issues found") rather than splitting diagnosis and remediation into two nodes -- the outcome depends on what the diagnostic step actually discovers, and a separate node presupposes there is necessarily something to fix.
CRITICAL RULE [EXACT COMMAND PRESERVATION]: When the DIRECTIVE contains a literal shell command -- introduced by phrases such as "Use execute_command to run exactly:", "run this command:", "execute:", or a fenced command block -- the corresponding node's objective MUST embed that exact command verbatim, including every URL, path, SHA, branch name, and flag. NEVER paraphrase a URL or path into prose. The Strategist generates actions from the node objective alone and cannot recover a URL that was paraphrased into "clone the repository". Paraphrasing a factual URL into a description is a data-loss bug, not a style preference. Example: if the DIRECTIVE says `run exactly: git clone https://github.com/Org/Repo.git /apps/repo`, the node objective MUST contain the string `git clone https://github.com/Org/Repo.git /apps/repo` verbatim, not "clone the Org/Repo repository to /apps/repo".

CRITICAL RULE [MULTI-STEP TOOL COMMANDS]: When the directive says "Using ONLY the execute_command tool" or "Using ONLY execute_command" and names specific shell commands (e.g. 'sed', 'grep', 'echo'), the node's success_criteria MUST include one CMD_OK per named command. For example, if the directive says "Step 1: create file X using execute_command. Step 2: use 'sed' to modify X", the success criteria MUST include both the creation command AND `CMD_OK` for the sed command. NEVER collapse multi-step directives into a single generic CMD_OK or CommandMarkerAcquired. Each named tool command is a separate mandatory criterion.
CRITICAL RULE [AUDIT SEARCH USES CHECKED FORM]: When the mission is an audit, analysis, survey, or report AND does not include a subsequent patch/fix/replace step on the searched code, emit every search criterion as SEARCH_CHECKED_OK:<path>:<term>, not SEARCH_CODE_OK:. A documented no-match is a VALID audit finding (e.g., 'no todo! macros in bins/'). Only use SEARCH_CODE_OK when a later node consumes the match to patch, replace, or rewrite existing code. When in doubt on an audit mission, prefer SEARCH_CHECKED_OK. Do NOT turn abstract prompt nouns (such as 'reliability', 'worker', or multi-word phrases like 'verify complete') into mandatory SEARCH_CHECKED_OK criteria—only emit SEARCH_CHECKED_OK for concrete code identifiers, macros, filenames, or explicit paths/terms named in the directive.\nCRITICAL RULE [CHECKED SEARCH EVIDENCE]: SEARCH_CHECKED_OK:<target-path>:<term> means the specified path/query must be searched successfully. A positive MATCHES result OR the executor's exact 'No code matches found in specified path.' outcome satisfies this criterion. Use SEARCH_CODE_OK when the term MUST be found. Never substitute SEARCH_CHECKED_OK for a positive-match requirement. Preserve the exact target path and query.
CRITICAL RULE [AUDIT / REPORT NODES — GROUNDING]:
When the current node’s objective is an analysis, survey, audit, review, or report
AND the node does NOT contain a subsequent patch / replace / rewrite step:

1. Any identifier, path, function name, struct name, or string that appears
   verbatim inside previously acquired READ_OK, CMD_OK, SEARCH_CODE_OK, or
   SEARCH_CHECKED_OK evidence is considered GROUNDED for descriptive use.

2. You may freely name those identifiers in the report. You do NOT need a
   `pub` definition, a public API claim, or an extra search to “prove” them.

3. The only things still forbidden are:
   - Inventing symbols that never appeared in the evidence
   - Claiming behaviour that the evidence does not support
   - Treating private / local items as if they were part of a public API
     (you may still describe what the source does)

4. Prefer the exact wording and line references that appear in the evidence
   over paraphrasing.

5. If the report must mention a symbol that is only present as a call site
   or local binding, simply describe the observed behaviour. Do not claim
   it is a public item.

This rule takes precedence over any general “must prove public definition”
grounding check for pure audit/report nodes.

CRITICAL RULE [AUDIT / REPORT NODES]: When the node objective is to produce an analysis, survey, audit, or report (and contains no subsequent patch/replace step), identifiers that appear verbatim in previously acquired READ_OK, CMD_OK, or SEARCH_CODE_OK evidence MAY be referenced descriptively. They do NOT need to be proven as `pub` items. Only inventing new public APIs or claiming un-evidenced behaviour is forbidden.

CRITICAL RULE [TARGET-PATH BINDING FOR CODE SEARCHES]: This rule governs the <target-path> field of BOTH `SEARCH_CODE_OK:` and `SEARCH_CHECKED_OK:` criteria; the [AUDIT SEARCH USES CHECKED FORM] rule above determines which form to use. A bare SEARCH_CODE_OK:<term> criterion defaults the search root to crates/runtime_core/src inside the local workspace, which is only correct for self-audits. Whenever the mission's target code lives in a different directory, the criterion MUST use the form SEARCH_CODE_OK:<target-path>:<term> OR SEARCH_CHECKED_OK:<target-path>:<term> (per the [AUDIT SEARCH USES CHECKED FORM] rule above), where <target-path> is copied verbatim from the mission text. The target path may be absolute (for example an entry starting with a single forward slash) or workspace-relative (for example a path starting with vendor/, apps/, bins/, or drones/). Do not invent a path, and do not reuse any path that appears elsewhere in this instruction: use the exact path the mission names. If the mission says the clone lives at vendor/external_repo, the criterion is SEARCH_CODE_OK:vendor/external_repo:<term>. If the mission says the clone lives at an absolute path, use that absolute path verbatim.
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
            "generationConfig": { "temperature": 0.0, "maxOutputTokens": 8192 }
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

        let result = match text_res {
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
        };

        // Single normalization point for every successfully-generated
        // graph, whether from the model or the fallback path. Both
        // rewrite helpers are applied here so a future early-return
        // branch that bypasses this match is the only way to skip
        // normalization.
        let result = result.map(|mut graph| {
            apply_read_only_search_rewrite(&mut graph, directive);
            apply_audit_search_rewrite(&mut graph, directive);
            drop_abstract_checked_criteria(&mut graph);
            apply_audit_recon_floor(&mut graph, directive);
            apply_write_deliverable_criterion(&mut graph, directive);
            apply_report_deliverable_criterion(&mut graph, directive);
            graph
        });

        result
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

    fn objective_requires_command(objective: &str) -> bool {
        let lower = objective.to_lowercase();
        lower.contains("command")
            || lower.contains("execute ")
            || lower.starts_with("run ")
            || crate::completion_engine::any_word_unnegated(&lower, &["execute", "run"])
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

            if node
                .success_criteria
                .iter()
                .any(|criterion| criterion.trim() == "NODE_OBJECTIVE_EXECUTED_AND_VERIFIED")
            {
                return Err(anyhow::anyhow!(
                    "Planner produced invalid self-attestation criterion for node '{}': NODE_OBJECTIVE_EXECUTED_AND_VERIFIED is not independent verification evidence.",
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
                }) || Self::objective_requires_command(&node.objective);

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

        // "ALL/ANY" is matched textually on purpose.
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

        // Software vocabulary that should *not* kill a pure algebra request.
        // We only treat a directive as software-oriented when it is primarily
        // about source inspection / repository work, not when it merely
        // mentions "script", "enum", or "write" as the vehicle for the math.
        let software_word = crate::completion_engine::any_word_unnegated(
            lower.as_str(),
            &[
                "struct",
                "trait",
                "crate",
                "module",
                "function",
                "source",
                "implementation",
                "repository",
                "code",
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

        let software_path =
            [".rs", ".toml", ".yaml", ".yml", "src/", "crates/", "apps/", "cargo.toml"]
                .iter()
                .any(|term| lower.contains(term));

        let software_phrase = ["lookup table", "hash table", "routing table", "state machine"]
            .iter()
            .any(|term| lower.contains(term));

        let software_artifact = software_word || software_path || software_phrase;

        // Source-review verbs still suppress the fast path.
        let source_review = crate::completion_engine::any_word_unnegated(
            lower.as_str(),
            &["inspect", "inspection", "review", "audit", "examine"],
        );

        // Already-supplied tables must not be re-derived.
        let table_phrase = ["all table", "any table", "truth table", "full 6x6", "6x6 table"]
            .iter()
            .any(|term| lower.contains(term));
        let supply_verb = ["supplied", "provided", "given", "shown", "below"]
            .iter()
            .any(|term| lower.contains(term));
        let supplied_results = table_phrase && supply_verb;

        // Explicit "write a script that verifies the algebra" is still math.
        // Only pure software / source-review directives are excluded.
        // A focused Assessment algebra verification is allowed to inspect
        // the supplied table, even when the directive says "reviewing".
        // Preserve the exclusion for ordinary "review the Assessment" tasks.
        let assessment_algebra_verification = action
            && mathematical_structure
            && lower.contains("assessment")
            && lower.contains("all")
            && lower.contains("any")
            && (lower.contains("commutative")
                || lower.contains("idempotent")
                || lower.contains("duality"))
            && !lower.contains("review the assessment")
            && !software_artifact;

        if assessment_algebra_verification {
            return true;
        }

        action
            && mathematical_structure
            && !software_artifact
            && !source_review
            && !supplied_results
    }

    fn create_mathematical_verification_graph(directive: &str) -> TaskGraph {
        use nexus_evidence_requirements::Assessment;

        // Capture the actual canonical implementation as independent input.
        let canonical_all: Vec<Vec<String>> = Assessment::STATES
            .iter()
            .map(|a| Assessment::STATES.iter().map(|b| format!("{:?}", a.all(*b))).collect())
            .collect();

        let dual_states: Vec<String> =
            Assessment::STATES.iter().map(|state| format!("{:?}", state.dual())).collect();

        let directive_json =
            serde_json::to_string(directive).expect("directive JSON serialization");
        let canonical_json =
            serde_json::to_string(&canonical_all).expect("canonical table JSON serialization");
        let dual_json = serde_json::to_string(&dual_states).expect("dual map JSON serialization");

        // The Python program parses the ALL table from the original directive.
        // It does not silently replace that table with the Rust implementation.
        let mut python = r###"
import sys

states = ["SC", "SP", "RC", "RP", "NP", "NN"]
index = {state: i for i, state in enumerate(states)}

directive = __DIRECTIVE_JSON__
canonical_all = __CANONICAL_JSON__
dual_values = __DUAL_JSON__
dual = dict(zip(states, dual_values))

def parse_supplied_all(text):
    lines = text.splitlines()
    marker = next(
        (i for i, line in enumerate(lines)
         if "ALL table" in line.lower()),
        None,
    )
    if marker is None:
        raise ValueError("Could not find the supplied ALL table.")

    header = None
    for i in range(marker + 1, len(lines)):
        if lines[i].strip().split() == states:
            header = i
            break

    if header is None:
        raise ValueError("Could not find the six-state ALL table header.")

    rows = []
    cursor = header + 1
    while cursor < len(lines) and len(rows) < len(states):
        tokens = lines[cursor].strip().split()
        cursor += 1
        if not tokens:
            continue
        if len(tokens) != 7:
            raise ValueError(
                "Malformed ALL row; expected a row label and six values: "
                + " ".join(tokens)
            )
        if tokens[0] != states[len(rows)]:
            raise ValueError(
                "Unexpected ALL row label: " + tokens[0]
            )
        if any(value not in states for value in tokens[1:]):
            raise ValueError("Unknown state in supplied ALL table.")
        rows.append(tokens[1:])

    if len(rows) != 6:
        raise ValueError("Supplied ALL table does not contain six rows.")
    return rows

def derive_any(all_table):
    return [
        [
            dual[
                all_table[index[dual[a]]][index[dual[b]]]
            ]
            for b in states
        ]
        for a in states
    ]

def properties(all_table):
    any_table = derive_any(all_table)

    all_commutative = all(
        all_table[i][j] == all_table[j][i]
        for i in range(6) for j in range(6)
    )
    all_idempotent = all(
        all_table[i][i] == states[i] for i in range(6)
    )
    rc_absorbing = all(
        all_table[index["RC"]][i] == "RC"
        and all_table[i][index["RC"]] == "RC"
        for i in range(6)
    )

    rank = {"SC": 0, "SP": 1, "NP": 2, "NN": 3}
    non_rejection = ["SC", "SP", "NP", "NN"]
    ordering = all(
        all_table[index[a]][index[b]]
        == min((a, b), key=lambda state: rank[state])
        for a in non_rejection for b in non_rejection
    )

    any_commutative = all(
        any_table[i][j] == any_table[j][i]
        for i in range(6) for j in range(6)
    )
    any_idempotent = all(
        any_table[i][i] == states[i] for i in range(6)
    )
    sc_absorbing = all(
        any_table[index["SC"]][i] == "SC"
        and any_table[i][index["SC"]] == "SC"
        for i in range(6)
    )

    duality_failures = []
    for a in states:
        for b in states:
            lhs = dual[any_table[index[a]][index[b]]]
            rhs = all_table[index[dual[a]]][index[dual[b]]]
            if lhs != rhs:
                duality_failures.append((a, b, lhs, rhs))

    dual_involution = all(dual[dual[state]] == state for state in states)

    return any_table, {
        "ALL_COMMUTATIVE": all_commutative,
        "ALL_IDEMPOTENT": all_idempotent,
        "RC_ABSORBING_ALL": rc_absorbing,
        "ORDER_SC_GT_SP_GT_NP_GT_NN": ordering,
        "ANY_COMMUTATIVE": any_commutative,
        "ANY_IDEMPOTENT": any_idempotent,
        "SC_ABSORBING_ANY": sc_absorbing,
        "DUALITY_ALL_36": len(duality_failures) == 0,
        "DUAL_INVOLUTION": dual_involution,
        "DUALITY_FAILURES": duality_failures,
    }

def print_table(label, table):
    print(label)
    print("    " + " ".join(states))
    for state, row in zip(states, table):
        print(state + "  " + " ".join(row))

def print_properties(label, values):
    print(label)
    for key, value in values.items():
        if key != "DUALITY_FAILURES":
            print(key + " " + str(value))
    if values["DUALITY_FAILURES"]:
        print("DUALITY_FAILURE_DETAILS", values["DUALITY_FAILURES"])

try:
    supplied_all = parse_supplied_all(directive)
except Exception as exc:
    print("VERIFICATION_INPUT_ERROR", str(exc))
    sys.exit(2)

if len(canonical_all) != 6 or any(len(row) != 6 for row in canonical_all):
    print("VERIFICATION_INPUT_ERROR Invalid canonical Rust table dimensions.")
    sys.exit(2)

if set(dual) != set(states) or any(value not in states for value in dual.values()):
    print("VERIFICATION_INPUT_ERROR Invalid canonical dual map.")
    sys.exit(2)

supplied_any, supplied_properties = properties(supplied_all)
canonical_any, canonical_properties = properties(canonical_all)

mismatches = []
for i, a in enumerate(states):
    for j, b in enumerate(states):
        supplied = supplied_all[i][j]
        canonical = canonical_all[i][j]
        if supplied != canonical:
            mismatches.append((a, b, supplied, canonical))

print_table("ALL TABLE", supplied_all)
print_table("ANY TABLE", supplied_any)
print_properties("SUPPLIED TABLE PROPERTIES", supplied_properties)

print_table("CANONICAL RUST ALL TABLE", canonical_all)
print_table("CANONICAL RUST ANY TABLE", canonical_any)
print_properties("CANONICAL RUST PROPERTIES", canonical_properties)

print("SUPPLIED_RUST_ALL_MISMATCHES", len(mismatches))
for row, column, supplied, canonical in mismatches:
    print(
        "TABLE_MISMATCH",
        "row=" + row,
        "column=" + column,
        "supplied=" + supplied,
        "rust=" + canonical,
    )

# A contradiction is a verification result, not an execution failure.
# Exit successfully after all input and algebra checks have completed.
print("VERIFICATION_EXECUTED True")
print("VERIFICATION_COMPLETE True")
"###
        .to_string();

        python = python.replace("__DIRECTIVE_JSON__", &directive_json);
        python = python.replace("__CANONICAL_JSON__", &canonical_json);
        python = python.replace("__DUAL_JSON__", &dual_json);

        let command = format!(
            "python3 - <<'ATHENAFUSIONX_MATH_VERIFY_EOF'\n{}\nATHENAFUSIONX_MATH_VERIFY_EOF",
            python
        );

        let objective = format!(
            "MATHEMATICAL_EXECUTION_VERIFIED: Execute `{}`. Parse and preserve the ALL table supplied in the directive, independently compare all 36 cells against the canonical Rust Assessment::all() table, derive ANY through duality, and calculate every requested algebraic property for both tables. Print complete tables, property results, and every mismatch. A mathematical contradiction is a valid verification finding; do not conceal it or convert it into an execution failure. DIRECTIVE: {}",
            command,
            directive
        );

        let mut nodes = std::collections::HashMap::new();
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

        if Self::objective_requires_command(directive) {
            return Ok(vec!["CMD_OK".to_string()]);
        }

        let read_only = crate::completion_engine::SubgoalCompletionEngine::infer_contract(
            "planner_fallback",
            directive,
        )
        .read_only;

        let trim_tok = |token: &str| -> String {
            token
                .trim_matches(|c: char| {
                    matches!(c, '`' | '"' | '\'' | ',' | ';' | ':' | ')' | '(' | '.')
                })
                .to_string()
        };

        if !read_only {
            for marker in ["write ", "save ", "export ", "create "] {
                if let Some(pos) = lower.find(marker) {
                    let remainder = directive[pos + marker.len()..].trim();
                    if let Some(path) = remainder
                        .split_whitespace()
                        .map(trim_tok)
                        .find(|token| token.contains('.') && !token.starts_with("the"))
                    {
                        return Ok(vec![format!("WRITE_OK:{}", path)]);
                    }
                }
            }
        }

        for marker in ["read ", "inspect "] {
            if let Some(pos) = lower.find(marker) {
                let remainder = directive[pos + marker.len()..].trim();
                if let Some(path) = remainder
                    .split_whitespace()
                    .map(trim_tok)
                    .find(|token| token.contains('/') || token.ends_with(".rs"))
                {
                    return Ok(vec![format!("READ_OK:{}", path)]);
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
        let directive = "Verify the router source contract using read-only source inspection.";

        assert!(!PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn assessment_algebra_review_with_supplied_table_uses_verification_fast_path() {
        let directive = r#"You are reviewing the Assessment enum from nexus_evidence_requirements.

States: SC, SP, RC, RP, NP, NN

ALL table (rows = left, cols = right):
        SC  SP  RC  RP  NP  NN
SC      SC  SP  RC  RP  NP  NN
SP      SP  SP  RC  RP  RP  RP
RC      RC  RC  RC  RC  RC  RC
RP      RP  RP  RC  RP  RP  RP
NP      NP  NP  RC  RP  NP  NN
NN      NN  NN  RC  RP  NN  NN

Tasks:
1. Verify ALL is commutative and idempotent.
2. Verify RC is absorbing for ALL.
3. Verify SC > SP > NP > NN ordering for non-rejection states.
4. Compute the full 6x6 ANY table using duality.
5. Check ANY is commutative and idempotent.
6. Verify SC is absorbing for ANY.
7. Check duality for all 36 pairs.
8. Report contradictions or edge cases."#;

        assert!(PlannerAgent::is_mathematical_derivation_directive(directive));

        let graph = PlannerAgent::create_mathematical_verification_graph(directive);
        let node = graph.nodes.get("task_1").expect("math verification node");

        assert_eq!(node.success_criteria, vec!["CMD_OK".to_string()]);
        assert!(node.objective.contains("SUPPLIED_RUST_ALL_MISMATCHES"));
        assert!(node.objective.contains("VERIFICATION_EXECUTED"));
        assert!(!node.objective.contains("WRITE_OK:"));
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
        let directive = "Derive the six-state ALL/ANY truth table and verify its duality.";

        assert!(PlannerAgent::is_mathematical_derivation_directive(directive));
    }

    #[test]
    fn mathematical_derivation_creates_exact_deterministic_cmd_strategy() {
        let directive = "Verify the six-state ALL/ANY algebra and compute the full 6x6 ANY table.";

        let graph = PlannerAgent::create_mathematical_verification_graph(directive);
        let node = &graph.nodes["task_1"];

        assert!(
            node.objective.starts_with(
                "MATHEMATICAL_EXECUTION_VERIFIED: Execute `python3 - <<'ATHENAFUSIONX_MATH_VERIFY_EOF'"
            ),
            "mathematical nodes must contain an explicit mathematical execution contract and executable Python heredoc"
        );
        assert_eq!(node.success_criteria, vec!["CMD_OK".to_string()]);

        let command = crate::completion_engine::extract_required_command(&node.objective)
            .expect("mathematical node must expose an exact executable command");

        assert!(command.starts_with("python3 - <<'ATHENAFUSIONX_MATH_VERIFY_EOF'"));
        assert!(command.contains("ALL TABLE"));
        assert!(command.contains("ANY TABLE"));
        assert!(command.contains("DUALITY_ALL_36"));

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

        let inferred = crate::completion_engine::SubgoalCompletionEngine::infer_contract(
            &node.id,
            &node.objective,
        );

        assert_eq!(
            inferred.conditions,
            vec![crate::completion_engine::CompletionCondition::MathematicalExecutionVerified {
                command,
            }],
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
    fn planner_rejects_legacy_self_attestation_criterion() {
        let result = parse(
            r#"{
                "nodes": {
                    "task_1": {
                        "id": "task_1",
                        "deps": [],
                        "required_agent": "CodeAnalyzer",
                        "objective": "Read router.rs",
                        "success_criteria": ["NODE_OBJECTIVE_EXECUTED_AND_VERIFIED"]
                    }
                }
            }"#,
        );

        assert!(
            result.is_err(),
            "planner must reject a criterion that merely attests to its own completion marker"
        );
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

        let dotted = PlannerAgent::fallback_success_criteria("Write mission_04_worker_recovery.md.")
            .expect("write fallback should trim trailing sentence period");
        assert_eq!(dotted, vec!["WRITE_OK:mission_04_worker_recovery.md"]);
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

/// The loose "search" + "memory" heuristic is meant for short one-line requests
/// ("search memory for X"). A long structured mission mentions both words in
/// prose ("do not use semantic memory", "openfang-memory", "search the source")
/// and must reach the LLM planner instead of collapsing into one
/// SEARCH_MEMORY_OK node.
fn is_short_plain_directive(d: &str) -> bool {
    let t = d.trim();
    t.len() <= 400 && !t.starts_with('#') && !t.contains("\n## ") && t.lines().count() <= 6
}

#[cfg(test)]
mod short_directive_tests {
    use super::is_short_plain_directive;

    #[test]
    fn short_requests_still_qualify() {
        assert!(is_short_plain_directive("Search memory for the last roadmap report"));
        assert!(is_short_plain_directive("Use SearchMemory to recall the openfang notes."));
    }

    #[test]
    fn long_structured_missions_do_not() {
        let mission =
            "# MISSION: X\n\n## Objective\n\nSearch the checkout. Do not use semantic memory.\n";
        assert!(!is_short_plain_directive(mission));
        assert!(!is_short_plain_directive(&"search memory ".repeat(40)));
        assert!(!is_short_plain_directive("a\nb\nc\nd\ne\nf\ng search memory"));
    }
}

#[cfg(test)]
mod audit_rewrite_tests {
    use super::*;
    use crate::orchestrator_v2::task_graph::{TaskExecutionState, TaskGraph, TaskNode, TaskStatus};
    use std::collections::HashMap;

    fn node_with(criteria: &[&str]) -> TaskNode {
        TaskNode {
            id: "t".to_string(),
            deps: vec![],
            required_agent: "CodeAnalyzer".to_string(),
            status: TaskStatus::Pending,
            objective: "test".to_string(),
            success_criteria: criteria.iter().map(|s| s.to_string()).collect(),
            execution_state: TaskExecutionState::default(),
            output_evidence: None,
            priority: 1,
        }
    }

    fn graph_with(criteria: &[&str]) -> TaskGraph {
        let mut g = TaskGraph { nodes: HashMap::new() };
        g.nodes.insert("t".to_string(), node_with(criteria));
        g
    }

    // ---- intent detection ---------------------------------------------

    #[test]
    fn audit_word_triggers_rewrite() {
        let mut g = graph_with(&["SEARCH_CODE_OK:bins:todo!"]);
        apply_audit_search_rewrite(&mut g, "Audit the codebase for stubs.");
        assert_eq!(g.nodes["t"].success_criteria, vec!["SEARCH_CHECKED_OK:bins:todo!".to_string()]);
    }

    #[test]
    fn survey_and_inventory_also_trigger() {
        let mut g = graph_with(&["SEARCH_CODE_OK:bins:todo!"]);
        apply_audit_search_rewrite(&mut g, "Survey the crates for unused code.");
        assert!(g.nodes["t"].success_criteria[0].starts_with("SEARCH_CHECKED_OK:"));
    }

    #[test]
    fn report_alone_does_not_trigger_rewrite() {
        // "Write a report about X" is NOT an audit if the search is
        // expected to produce the report's content.
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:foo"]);
        apply_audit_search_rewrite(&mut g, "Write a report about the foo module.");
        assert_eq!(g.nodes["t"].success_criteria, vec!["SEARCH_CODE_OK:crates:foo".to_string()]);
    }

    #[test]
    fn analysis_alone_does_not_trigger_rewrite() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:foo"]);
        apply_audit_search_rewrite(&mut g, "Analyze the foo module and produce a summary.");
        assert_eq!(g.nodes["t"].success_criteria, vec!["SEARCH_CODE_OK:crates:foo".to_string()]);
    }

    #[test]
    fn non_audit_find_keeps_strict() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:foo"]);
        apply_audit_search_rewrite(&mut g, "Find all callers of foo.");
        assert_eq!(g.nodes["t"].success_criteria, vec!["SEARCH_CODE_OK:crates:foo".to_string()]);
    }

    // ---- must-find override --------------------------------------------

    #[test]
    fn must_find_overrides_audit_intent() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:deprecated_fn"]);
        apply_audit_search_rewrite(&mut g, "Audit the code to locate every deprecated_fn usage.");
        assert_eq!(
            g.nodes["t"].success_criteria,
            vec!["SEARCH_CODE_OK:crates:deprecated_fn".to_string()]
        );
    }

    #[test]
    fn find_all_overrides_audit_intent() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:foo"]);
        apply_audit_search_rewrite(&mut g, "Audit for foo, then find all usages of bar.");
        assert_eq!(g.nodes["t"].success_criteria, vec!["SEARCH_CODE_OK:crates:foo".to_string()]);
    }

    // ---- mutation gate -------------------------------------------------

    #[test]
    fn patch_like_criteria_gate_the_rewrite() {
        let mut g =
            graph_with(&["SEARCH_CODE_OK:crates:todo!", "PATCH_FUNCTION_OK:crates/foo.rs:bar"]);
        apply_audit_search_rewrite(&mut g, "Audit and fix all stubs.");
        assert!(
            g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")),
            "PATCH_FUNCTION_OK must gate the rewrite"
        );
    }

    #[test]
    fn diff_patch_gates_the_rewrite() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:todo!", "DIFF_PATCH_OK:crates/foo.rs"]);
        apply_audit_search_rewrite(&mut g, "Audit and fix stubs.");
        assert!(g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")));
    }

    #[test]
    fn replace_lines_gates_the_rewrite() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:todo!", "REPLACE_LINES_OK:crates/foo.rs"]);
        apply_audit_search_rewrite(&mut g, "Audit and fix stubs.");
        assert!(g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")));
    }

    #[test]
    fn write_rs_path_gates_the_rewrite() {
        let mut g = graph_with(&[
            "SEARCH_CODE_OK:crates:todo!",
            "WRITE_OK:crates/runtime_core/src/new_file.rs",
        ]);
        apply_audit_search_rewrite(&mut g, "Audit and create the new source file.");
        assert!(g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")));
    }

    #[test]
    fn write_report_does_not_gate_the_rewrite() {
        let mut g = graph_with(&["SEARCH_CODE_OK:bins:todo!", "WRITE_OK:full_system_audit.md"]);
        apply_audit_search_rewrite(&mut g, "Audit the codebase and write a report.");
        assert!(
            g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CHECKED_OK:")),
            "report write must NOT gate the audit rewrite"
        );
        assert!(
            g.nodes["t"].success_criteria.iter().any(|c| c == "WRITE_OK:full_system_audit.md"),
            "the report write criterion itself must be preserved"
        );
    }

    #[test]
    fn no_search_criteria_is_noop() {
        let mut g = graph_with(&["READ_OK:foo.rs"]);
        apply_audit_search_rewrite(&mut g, "Audit the codebase.");
        assert_eq!(g.nodes["t"].success_criteria, vec!["READ_OK:foo.rs".to_string()]);
    }

    // ---- report deliverable criterion ----------------------------------

    const INLINE_AUDIT: &str =
        "READ-ONLY audit. Do not modify files. Return the final report in the response.";

    #[test]
    fn inline_report_audit_gets_analysis_ok_on_search_only_node() {
        let mut g = graph_with(&["SEARCH_CHECKED_OK:bins/:main", "SEARCH_CHECKED_OK:apps/:main"]);
        apply_report_deliverable_criterion(&mut g, INLINE_AUDIT);
        assert_eq!(g.nodes["t"].success_criteria.last().unwrap(), "ANALYSIS_OK");
        assert_eq!(g.nodes["t"].success_criteria.len(), 3);
    }

    #[test]
    fn report_criterion_is_not_duplicated() {
        let mut g = graph_with(&["READ_OK:a.rs", "ANALYSIS_OK"]);
        apply_report_deliverable_criterion(&mut g, INLINE_AUDIT);
        assert_eq!(g.nodes["t"].success_criteria, vec!["READ_OK:a.rs", "ANALYSIS_OK"]);
    }

    #[test]
    fn report_criterion_skips_write_and_command_nodes_and_plain_directives() {
        let mut w = graph_with(&["READ_OK:a.rs", "WRITE_OK:r.md"]);
        apply_report_deliverable_criterion(&mut w, INLINE_AUDIT);
        assert!(!w.nodes["t"].success_criteria.iter().any(|c| c == "ANALYSIS_OK"));

        let mut c = graph_with(&["CMD_OK"]);
        apply_report_deliverable_criterion(&mut c, INLINE_AUDIT);
        assert_eq!(c.nodes["t"].success_criteria, vec!["CMD_OK"]);

        let mut p = graph_with(&["READ_OK:a.rs"]);
        apply_report_deliverable_criterion(&mut p, "Read a.rs and summarize it.");
        assert_eq!(p.nodes["t"].success_criteria, vec!["READ_OK:a.rs"]);
    }

    #[test]
    fn report_criterion_goes_on_the_sink_node_only() {
        let mut g = graph_with(&["READ_OK:a.rs"]);
        let mut second = node_with(&["READ_OK:b.rs"]);
        second.id = "u".to_string();
        second.deps = vec!["t".to_string()];
        g.nodes.insert("u".to_string(), second);
        apply_report_deliverable_criterion(&mut g, INLINE_AUDIT);
        assert_eq!(g.nodes["t"].success_criteria, vec!["READ_OK:a.rs"]);
        assert_eq!(g.nodes["u"].success_criteria.last().unwrap(), "ANALYSIS_OK");
    }

    #[test]
    fn return_a_report_directive_triggers_inline_criterion() {
        assert!(directive_delivers_report_inline(
            "Return a structured audit report containing: 1. Executive summary. 2. Findings."
        ));
    }

    #[test]
    fn deliver_a_report_directive_triggers_inline_criterion() {
        assert!(directive_delivers_report_inline(
            "Deliver a comprehensive findings report in the final response."
        ));
    }

    #[test]
    fn write_report_to_disk_does_not_trigger_inline() {
        assert!(!directive_delivers_report_inline(
            "Write a report to disk at /tmp/audit_report.md."
        ));
        assert!(!directive_delivers_report_inline("Produce a report file named audit.json."));
    }

    #[test]
    fn return_a_report_to_a_file_does_not_trigger_inline() {
        assert!(!directive_delivers_report_inline("Return a report to the file audit_report.md."));
    }

    // ---- structural guard (call-site presence only, NOT routing) -------

    /// Guards against accidental removal of the call site. This is a
    /// source-presence check, not a routing proof -- the behavioral
    /// coverage of `apply_audit_search_rewrite` is provided by the tests
    /// above; the integration of the wrapper into `generate_plan` is
    /// covered by the `.map(...)` presence check just below.
    #[test]
    fn both_rewrites_have_call_sites() {
        let source = include_str!("planner.rs");
        assert!(
            source.contains("apply_read_only_search_rewrite(&mut graph, directive);"),
            "apply_read_only_search_rewrite must have a call site"
        );
        assert!(
            source.contains("apply_audit_search_rewrite(&mut graph, directive);"),
            "apply_audit_search_rewrite must have a call site"
        );
    }

    /// Guards the plumbing: the outer `match text_res { ... }` in
    /// `generate_plan` must wrap into `.map(|mut graph| { ... })`. This
    /// is a source-shape check; it does not prove execution order, but a
    /// refactor that removes the wrapper will break this test.
    #[test]
    fn generate_plan_wraps_result_through_rewrite_map() {
        let source = include_str!("planner.rs");
        assert!(
            source.contains("let result = result.map(|mut graph| {"),
            "generate_plan must wrap its result through .map(|mut graph| {{ ... }})"
        );
    }

    // ---- read-only rewrite regression coverage -------------------------

    fn two_node_graph_with_source_mutation() -> TaskGraph {
        let mut g = TaskGraph { nodes: HashMap::new() };
        g.nodes.insert(
            "a".to_string(),
            TaskNode {
                id: "a".to_string(),
                deps: vec![],
                required_agent: "CodeAnalyzer".to_string(),
                status: TaskStatus::Pending,
                objective: "search".to_string(),
                success_criteria: vec!["SEARCH_CODE_OK:crates:deprecated_fn".to_string()],
                execution_state: TaskExecutionState::default(),
                output_evidence: None,
                priority: 1,
            },
        );
        g.nodes.insert(
            "b".to_string(),
            TaskNode {
                id: "b".to_string(),
                deps: vec![],
                required_agent: "CodeAnalyzer".to_string(),
                status: TaskStatus::Pending,
                objective: "patch".to_string(),
                success_criteria: vec!["PATCH_FUNCTION_OK:crates/foo.rs:fix".to_string()],
                execution_state: TaskExecutionState::default(),
                output_evidence: None,
                priority: 2,
            },
        );
        g
    }

    #[test]
    fn read_only_plus_must_find_keeps_strict() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:deprecated_fn"]);
        apply_read_only_search_rewrite(&mut g, "READ-ONLY. Find all usages of deprecated_fn.");
        assert_eq!(
            g.nodes["t"].success_criteria,
            vec!["SEARCH_CODE_OK:crates:deprecated_fn".to_string()],
            "read-only + must-find must keep strict"
        );
    }

    #[test]
    fn read_only_plus_source_mutation_elsewhere_keeps_strict() {
        let mut g = two_node_graph_with_source_mutation();
        apply_read_only_search_rewrite(&mut g, "READ-ONLY. Do not modify any files.");
        assert!(
            g.nodes["a"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")),
            "graph-wide mutation guard must leave node A strict"
        );
    }

    #[test]
    fn read_only_plus_report_artifact_keeps_strict() {
        // READ-ONLY contract: any WRITE_OK: criterion -- including a
        // report artifact -- is a prompt violation. The read-only rewrite
        // must not weaken a search in a malformed graph; the audit
        // helper (which does allow report writes) is the one that
        // distinguishes .md from .rs.
        let mut g = graph_with(&["SEARCH_CODE_OK:bins:todo!", "WRITE_OK:audit_report.md"]);
        apply_read_only_search_rewrite(&mut g, "READ-ONLY. Do not modify any files.");
        assert!(
            g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")),
            "READ-ONLY must keep strict when any WRITE_OK: is present"
        );
    }

    #[test]
    fn read_only_ordinary_inspection_rewrites_to_checked() {
        let mut g = graph_with(&["SEARCH_CODE_OK:crates:todo!"]);
        apply_read_only_search_rewrite(&mut g, "READ-ONLY. Do not modify any files.");
        assert!(
            g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CHECKED_OK:")),
            "read-only ordinary inspection must rewrite to checked"
        );
    }

    #[test]
    fn audit_mission_plus_report_artifact_rewrites_to_checked() {
        // Audit policy: report writes do NOT gate the audit rewrite.
        // This is the mirror of read_only_plus_report_artifact_keeps_strict
        // and demonstrates that the two policies are intentionally distinct.
        let mut g = graph_with(&["SEARCH_CODE_OK:bins:todo!", "WRITE_OK:full_system_audit.md"]);
        apply_audit_search_rewrite(&mut g, "Audit the codebase and write a report.");
        assert!(
            g.nodes["t"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CHECKED_OK:")),
            "audit + report write must rewrite to checked"
        );
    }

    #[test]
    fn combined_wrapper_preserves_strict_for_source_mutation_graph() {
        let mut g = two_node_graph_with_source_mutation();
        let directive = "READ-ONLY. Audit the crates.";
        apply_read_only_search_rewrite(&mut g, directive);
        apply_audit_search_rewrite(&mut g, directive);
        assert!(
            g.nodes["a"].success_criteria.iter().any(|c| c.starts_with("SEARCH_CODE_OK:")),
            "combined wrapper must preserve strict when graph has source mutation"
        );
    }
}

#[cfg(test)]
mod audit_recon_floor_tests {
    use super::named_workspace_files;

    // cargo test runs with cwd = crates/runtime_core, which has a small Cargo.toml.
    #[test]
    fn names_existing_small_files_only() {
        let d = "Audit the repo: inspect Cargo.toml, write system_audit_report.md, find *.bak* files, see /etc/passwd and nope_missing.toml.";
        assert_eq!(named_workspace_files(d), vec!["Cargo.toml".to_string()]);
    }

    #[test]
    fn skips_reports_logs_locks_and_traversal() {
        let d = "Read report.md daemon.log Cargo.lock ../Cargo.toml and e.g. nothing.";
        assert!(named_workspace_files(d).is_empty());
    }

    #[test]
    fn deduplicates_repeated_mentions() {
        let d = "Audit: Cargo.toml, then Cargo.toml again (Cargo.toml).";
        assert_eq!(named_workspace_files(d), vec!["Cargo.toml".to_string()]);
    }
}

#[cfg(test)]
mod write_deliverable_tests {
    use super::extract_write_targets_from_directive;

    #[test]
    fn simple_write_is_extracted() {
        assert_eq!(
            extract_write_targets_from_directive("Write mission_04_worker_recovery.md."),
            vec!["mission_04_worker_recovery.md".to_string()]
        );
    }

    #[test]
    fn write_result_to_is_extracted() {
        assert_eq!(
            extract_write_targets_from_directive(
                "Write the result to mission_01_completion_audit.md."
            ),
            vec!["mission_01_completion_audit.md".to_string()]
        );
    }

    #[test]
    fn source_extension_is_not_a_report() {
        assert!(extract_write_targets_from_directive(
            "Write the implementation to crates/foo/src/lib.rs"
        ).is_empty());
    }

    #[test]
    fn absolute_path_is_rejected() {
        assert!(extract_write_targets_from_directive(
            "Write the report to /etc/passwd.md"
        ).is_empty());
    }

    #[test]
    fn traversal_is_rejected() {
        assert!(extract_write_targets_from_directive(
            "Write the report to ../etc/report.md"
        ).is_empty());
    }

    #[test]
    fn multiple_deliverables_are_extracted() {
        let out = extract_write_targets_from_directive(
            "Write findings to a.md. Then save the result to b.json."
        );
        assert!(out.contains(&"a.md".to_string()));
        assert!(out.contains(&"b.json".to_string()));
    }
}

#[cfg(test)]
mod directive_deliverable_presence_tests {
    use super::extract_write_targets_from_directive;

    fn missing(directive: &str) -> Vec<String> {
        extract_write_targets_from_directive(directive)
            .into_iter()
            .filter(|p| !std::path::Path::new(p).exists())
            .collect()
    }

    #[test]
    fn missing_deliverable_is_detected() {
        // A path with an unlikely extension, definitely not on disk.
        let missing = missing("Write to __impossible_deliverable_xyzzy.md");
        assert_eq!(missing, vec!["__impossible_deliverable_xyzzy.md".to_string()]);
    }

    #[test]
    fn multiple_missing_are_all_reported() {
        let m = missing("Write to __absent_a.md. Save the result to __absent_b.json.");
        assert!(m.contains(&"__absent_a.md".to_string()));
        assert!(m.contains(&"__absent_b.json".to_string()));
    }
}

/// True when a SEARCH_CHECKED_OK target's term is a concrete code
/// identifier, macro, filename, or explicit path -- not an abstract
/// prompt noun.  Enforces CRITICAL RULE [AUDIT SEARCH USES CHECKED FORM]
/// at minting time.  Parsing mirrors search_checked_satisfied
/// (evidence_markers.rs) so the two agree on what the "term" is.
/// Enforce the planner-prompt rule "do not turn abstract nouns into mandatory
/// SEARCH_CHECKED_OK criteria" in code. Drops non-concrete SEARCH_CHECKED_OK
/// criteria, but never empties a node: if every criterion would be dropped,
/// the node is left untouched (an empty contract fails closed).
fn filter_abstract_checked(criteria: &mut Vec<String>) {
    let keep: Vec<bool> = criteria
        .iter()
        .map(|c| match c.trim().strip_prefix("SEARCH_CHECKED_OK:") {
            Some(target) => checked_term_is_concrete(target),
            None => true,
        })
        .collect();
    if !keep.iter().any(|k| *k) {
        return;
    }
    let mut i = 0;
    criteria.retain(|_| {
        let k = keep[i];
        i += 1;
        k
    });
}

fn drop_abstract_checked_criteria(graph: &mut TaskGraph) {
    for node in graph.nodes.values_mut() {
        filter_abstract_checked(&mut node.success_criteria);
    }
}

fn checked_term_is_concrete(target: &str) -> bool {
    let t = target.trim();
    if t.is_empty() {
        return false;
    }
    let term = match t.split_once(':') {
        Some((p, rest))
            if crate::evidence_markers::looks_like_workspace_path(p.trim()) =>
        {
            rest.trim()
        }
        _ => t,
    };
    if term.is_empty() {
        return false;
    }
    const ABSTRACT: &[&str] = &[
        "worker", "reliability", "security", "performance", "quality",
        "error", "errors", "bug", "bugs", "issue", "issues", "risk",
        "risks", "behavior", "behaviour", "logic", "flow", "state",
        "verify", "complete", "check", "test", "audit", "review",
        "status", "health", "coverage", "safety", "robustness",
    ];
    // Multi-word: concrete if ANY word is not abstract.
    // Single word:  concrete if not abstract.
    !term
        .split_whitespace()
        .all(|w| ABSTRACT.contains(&w.to_ascii_lowercase().as_str()))
}

#[cfg(test)]
mod noun_filter_tests {
    use super::checked_term_is_concrete;

    #[test]
    fn concrete_identifiers_pass() {
        assert!(checked_term_is_concrete("unimplemented!"));
        assert!(checked_term_is_concrete("tokio::spawn"));
        assert!(checked_term_is_concrete("Cargo.toml"));
        assert!(checked_term_is_concrete("crates/runtime_core/src"));
        assert!(checked_term_is_concrete("sanitize_in"));
        assert!(checked_term_is_concrete("SCREAMING_SNAKE"));
    }

    #[test]
    fn abstract_nouns_rejected() {
        assert!(!checked_term_is_concrete("worker"));
        assert!(!checked_term_is_concrete("reliability"));
        assert!(!checked_term_is_concrete("verify"));
        assert!(!checked_term_is_concrete("security"));
        assert!(!checked_term_is_concrete("robustness"));
    }

    #[test]
    fn multi_word_abstract_phrase_rejected() {
        assert!(!checked_term_is_concrete("verify complete"));
    }

    #[test]
    fn path_scoped_abstract_noun_is_rejected() {
        assert!(!checked_term_is_concrete("crates/runtime_core/src:worker"));
        assert!(!checked_term_is_concrete("bins/ide_backend:reliability"));
    }

    #[test]
    fn path_scoped_concrete_term_passes() {
        assert!(checked_term_is_concrete("crates/runtime_core/src:unimplemented!"));
        assert!(checked_term_is_concrete("crates/runtime_core/src:sanitize_in"));
    }

    #[test]
    fn empty_rejected() {
        assert!(!checked_term_is_concrete(""));
        assert!(!checked_term_is_concrete("  "));
        assert!(!checked_term_is_concrete("crates/runtime_core/src:"));
    }
}

#[cfg(test)]
mod p1b_abstract_checked_tests {
    use super::*;

    #[test]
    fn predicate_rejects_abstract_and_accepts_identifiers() {
        assert!(!checked_term_is_concrete("reliability"));
        assert!(!checked_term_is_concrete("verify complete"));
        assert!(!checked_term_is_concrete("crates/runtime_core/src:worker"));
        assert!(checked_term_is_concrete("crates/runtime_core/src:unimplemented!"));
        assert!(checked_term_is_concrete("deprecated_fn"));
    }

    #[test]
    fn abstract_checked_criterion_is_dropped_when_others_remain() {
        let mut v = vec![
            "SEARCH_CHECKED_OK:crates/runtime_core/src:worker".to_string(),
            "SEARCH_CHECKED_OK:crates/runtime_core/src:unimplemented!".to_string(),
            "ANALYSIS_OK".to_string(),
        ];
        filter_abstract_checked(&mut v);
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|c| !c.ends_with(":worker")));
    }

    #[test]
    fn last_criterion_is_never_dropped() {
        let mut v = vec!["SEARCH_CHECKED_OK:reliability".to_string()];
        filter_abstract_checked(&mut v);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn search_code_criteria_are_untouched() {
        let mut v = vec![
            "SEARCH_CODE_OK:crates:worker".to_string(),
            "SEARCH_CHECKED_OK:crates:todo!".to_string(),
        ];
        filter_abstract_checked(&mut v);
        assert_eq!(v.len(), 2);
    }
}
