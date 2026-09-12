use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompletionCondition {
    ReadMarkerAcquired,
    SearchMarkerAcquired,
    SearchMemoryMarkerAcquired,
    WriteMarkerAcquired,
    PatchMarkerAcquired,
    CommandMarkerAcquired,
    CommandExecutionVerified { command: String },
    LineageEvidenceAcquired,
    ReadFunctionMarkerAcquired { path: String, function_name: String },
    SourceCodeEvidenceRequired,
    MinBeliefScore(f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgoalContract {
    pub node_id: String,
    pub objective: String,
    pub conditions: Vec<CompletionCondition>,
    pub is_mutation_task: bool,
    #[serde(default)]
    pub required_artifact_paths: Vec<ArtifactSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveBudget {
    pub max_turns: u32,
    pub max_model_calls: u32,
    pub model_calls_made: u32,
}

impl CognitiveBudget {
    pub fn default_for_node() -> Self {
        Self { max_turns: 60, max_model_calls: 150, model_calls_made: 0 }
    }

    pub fn record_call(&mut self) {
        self.model_calls_made += 1;
    }

    pub fn is_exhausted(&self, current_turn: u32) -> bool {
        current_turn >= self.max_turns || self.model_calls_made >= self.max_model_calls
    }
}

pub struct SubgoalCompletionEngine;

fn has_word_unnegated(text: &str, word: &str) -> bool {
    let negations = ["not", "never", "no", "cannot", "n't"];
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (i, tok) in tokens.iter().enumerate() {
        let clean = tok.trim_matches(|c: char| !c.is_alphanumeric());
        let matches_word = clean.eq_ignore_ascii_case(word)
            || tok.split(|c: char| !c.is_alphanumeric()).any(|sub| sub.eq_ignore_ascii_case(word));

        if matches_word {
            let start = i.saturating_sub(3);
            let negated = tokens[start..i].iter().any(|w| {
                let wl = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                negations.contains(&wl.as_str()) || wl.ends_with("n't")
            });
            if !negated {
                return true;
            }
        }
    }
    false
}

pub(crate) fn any_word_unnegated(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| has_word_unnegated(text, w))
}

/// Recognize a read-only request for actual host metrics.
///
/// These observations are physical runtime state, not source-code analysis.
/// Keep this intentionally conjunctive so ordinary mentions of one metric
/// do not force shell execution.
/// Recognize a node whose objective explicitly requires inherited DAG
/// evidence instead of independently re-querying the source state.
pub(crate) fn is_lineage_inherit_only_objective(objective: &str) -> bool {
    let text = objective.to_lowercase();

    let has_dependency = text.contains("depend on")
        || text.contains("depends on")
        || text.contains("parent")
        || text.contains("dag lineage");

    let has_inherited_result = text.contains("execution result")
        || text.contains("inherited")
        || text.contains("received through");

    let forbids_rediscovery = text.contains("do not independently rediscover")
        || text.contains("do not independently re-query")
        || text.contains("do not independently requery")
        || text.contains("without re-querying")
        || text.contains("without requerying");

    has_dependency && has_inherited_result && forbids_rediscovery
}

pub(crate) fn is_host_observation_objective(objective: &str) -> bool {
    let text = objective.to_lowercase();

    if is_lineage_inherit_only_objective(objective) || text.contains("lineage mode: inherit_only") {
        return false;
    }

    let has_uptime = any_word_unnegated(&text, &["uptime"]);
    let has_filesystem = any_word_unnegated(&text, &["filesystem", "disk", "storage", "capacity"]);
    let has_memory = any_word_unnegated(&text, &["memory", "ram"]);
    let has_load = any_word_unnegated(&text, &["load"]);
    let explicit_search_memory = any_word_unnegated(&text, &["searchmemory", "search_memory"]);

    has_uptime && has_filesystem && has_memory && has_load && !explicit_search_memory
}

fn extract_required_command(objective: &str) -> Option<String> {
    let lower = objective.to_lowercase();

    // Prefer an explicit command following "command", enclosed in backticks.
    if let Some(command_pos) = lower.find("command") {
        let after = &objective[command_pos + "command".len()..];

        if let Some(start) = after.find('`') {
            let remainder = &after[start + 1..];
            if let Some(end) = remainder.find('`') {
                let command = remainder[..end].trim();
                if !command.is_empty() {
                    return Some(command.to_string());
                }
            }
        }

        // Also accept a command introduced by a simple quoted string.
        for quote in ['"', '\''] {
            if let Some(start) = after.find(quote) {
                let remainder = &after[start + quote.len_utf8()..];
                if let Some(end) = remainder.find(quote) {
                    let command = remainder[..end].trim();
                    if !command.is_empty() {
                        return Some(command.to_string());
                    }
                }
            }
        }
    }

    // Accept common direct forms such as:
    // Run `uptime; df -h /; free -m`
    // Execute `uptime`
    for marker in ["run `", "execute `"] {
        if let Some(start) = lower.find(marker) {
            let remainder = &objective[start + marker.len()..];
            if let Some(end) = remainder.find('`') {
                let command = remainder[..end].trim();
                if !command.is_empty() {
                    return Some(command.to_string());
                }
            }
        }
    }

    None
}

impl SubgoalCompletionEngine {
    pub fn infer_contract(node_id: &str, objective: &str) -> SubgoalContract {
        let obj_lower = objective.trim().to_lowercase();
        let classify_text = obj_lower
            .strip_prefix("objective:")
            .or_else(|| obj_lower.strip_prefix("objective"))
            .unwrap_or(&obj_lower)
            .trim();

        let mut conditions = Vec::new();
        let mut is_mutation_task = false;

        // Explicit READ_FUNCTION_OK contract:
        if let Some(pos) = objective.find("READ_FUNCTION_OK:") {
            let remainder = &objective[pos + "READ_FUNCTION_OK:".len()..];
            let target =
                remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');
            let mut parts = target.splitn(2, ':');
            let path = parts.next().unwrap_or("").trim().to_string();
            let func = parts.next().unwrap_or("").trim().to_string();
            if !path.is_empty() && !func.is_empty() {
                return SubgoalContract {
                    node_id: node_id.to_string(),
                    objective: objective.to_string(),
                    conditions: vec![CompletionCondition::ReadFunctionMarkerAcquired {
                        path,
                        function_name: func,
                    }],
                    is_mutation_task: false,
                    required_artifact_paths: Vec::new(),
                };
            }
        }

        if objective.contains("LINEAGE MODE: INHERIT_ONLY")
            || classify_text.contains("lineage mode: inherit_only")
            || is_lineage_inherit_only_objective(classify_text)
        {
            return SubgoalContract {
                node_id: node_id.to_string(),
                objective: objective.to_string(),
                conditions: vec![CompletionCondition::LineageEvidenceAcquired],
                is_mutation_task: false,
                required_artifact_paths: Vec::new(),
            };
        }

        // Native host observation is a physical execution contract.
        // It must take precedence over generic "inspect/investigate" source
        // analysis because the requested facts exist only in live host state.
        if is_host_observation_objective(classify_text) {
            return SubgoalContract {
                node_id: node_id.to_string(),
                objective: objective.to_string(),
                conditions: vec![CompletionCondition::CommandExecutionVerified {
                    command: "uptime; df -h /; free -m".to_string(),
                }],
                is_mutation_task: false,
                required_artifact_paths: Vec::new(),
            };
        }

        // Detect every material intent independently.
        //
        // A compound objective such as:
        //   read X, search Y, analyze findings, write Z
        //
        // must produce an AND contract:
        //   READ + SEARCH + WRITE + evidence threshold
        //
        // Previously the write/mutation branch was mutually exclusive with
        // reconnaissance, causing WRITE_OK to masquerade as full completion.

        let has_patch =
            any_word_unnegated(classify_text, &["patch", "apply", "fix", "remediate", "mutate"]);

        let has_write = any_word_unnegated(
            classify_text,
            &["write", "export", "save", "implement", "integrate", "add", "create", "synthesize"],
        );

        let has_read_intent =
            any_word_unnegated(classify_text, &["read", "inspect", "examine", "review", "open"]);

        let has_search_memory_intent =
            any_word_unnegated(classify_text, &["searchmemory", "search_memory"])
                || (any_word_unnegated(classify_text, &["search"])
                    && any_word_unnegated(classify_text, &["memory"])
                    && !is_host_observation_objective(classify_text));

        let has_search_intent =
            any_word_unnegated(classify_text, &["search", "find", "scan", "grep", "locate"])
                && !has_search_memory_intent;

        let has_analysis_intent = any_word_unnegated(
            classify_text,
            &[
                "analyze",
                "analyse",
                "analysis",
                "investigate",
                "audit",
                "compare",
                "comparison",
                "synthesize",
                "synthesis",
            ],
        );

        // EXPLICIT NATIVE TOOL CONTRACT:
        // SearchMemory is authoritative when explicitly requested. Do not
        // reinterpret the directive as filesystem search, source analysis,
        // command execution, or a generic belief-threshold task.
        if has_search_memory_intent {
            return SubgoalContract {
                node_id: node_id.to_string(),
                objective: objective.to_string(),
                conditions: vec![CompletionCondition::SearchMemoryMarkerAcquired],
                is_mutation_task: false,
                required_artifact_paths: Vec::new(),
            };
        }

        if has_patch {
            is_mutation_task = true;
            conditions.push(CompletionCondition::PatchMarkerAcquired);
        }

        if has_write {
            is_mutation_task = true;
            conditions.push(CompletionCondition::WriteMarkerAcquired);
        }

        if has_read_intent {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
        }

        if has_search_intent {
            conditions.push(CompletionCondition::SearchMarkerAcquired);
        }

        // Analysis/investigation over source code requires actual source
        // evidence. This is deliberately independent of WRITE_OK.
        if has_analysis_intent || (has_read_intent && has_search_intent) {
            conditions.push(CompletionCondition::SourceCodeEvidenceRequired);
        }

        // Recon-only objectives still need a belief threshold.
        // Compound mutation/recon objectives also retain a modest threshold
        // rather than allowing a write marker to manufacture certainty.
        let has_recon = has_read_intent || has_search_intent || has_analysis_intent;

        if has_recon {
            let threshold = if has_write || has_patch { 0.75 } else { 0.65 };
            conditions.push(CompletionCondition::MinBeliefScore(threshold));
        } else if !has_write && !has_patch {
            // Pure command objectives require physical evidence for the
            // exact requested command. A generic [CMD_OK] only proves that
            // some command succeeded and is therefore insufficient.
            if let Some(command) = extract_required_command(objective) {
                conditions.push(CompletionCondition::CommandExecutionVerified { command });
            } else {
                // Preserve the legacy contract only when the objective does
                // not expose a command string that can be bound deterministically.
                conditions.push(CompletionCondition::CommandMarkerAcquired);
            }
        }

        let arts = extract_required_artifact_paths(objective);
        if !arts.is_empty()
            && !conditions.iter().any(|c| {
                matches!(
                    c,
                    CompletionCondition::PatchMarkerAcquired
                        | CompletionCondition::WriteMarkerAcquired
                )
            })
        {
            conditions.push(CompletionCondition::WriteMarkerAcquired);
        }

        SubgoalContract {
            node_id: node_id.to_string(),
            objective: objective.to_string(),
            conditions,
            is_mutation_task,
            required_artifact_paths: arts,
        }
    }

    pub fn evaluate(contract: &SubgoalContract, local_evidence: &str, belief_score: f32) -> bool {
        let verifier_result = crate::outcome_verifier::OutcomeVerifier::verify(
            &contract.node_id,
            local_evidence,
            belief_score,
            &contract.conditions,
        );
        matches!(verifier_result, crate::outcome_verifier::CompletionStatus::Approved)
    }

    /// Returns true when every non-output condition required for
    /// reconnaissance has been satisfied.
    ///
    /// Write/Patch conditions are deliberately excluded because this
    /// gate authorizes the transition INTO the output phase.

    pub fn reconnaissance_complete(
        contract: &SubgoalContract,
        local_evidence: &str,
        belief_score: f32,
    ) -> bool {
        let recon_conditions: Vec<CompletionCondition> = contract
            .conditions
            .iter()
            .filter(|condition| {
                !matches!(
                    condition,
                    CompletionCondition::WriteMarkerAcquired
                        | CompletionCondition::PatchMarkerAcquired
                )
            })
            .cloned()
            .collect();

        let verifier_result = crate::outcome_verifier::OutcomeVerifier::verify(
            &contract.node_id,
            local_evidence,
            belief_score,
            &recon_conditions,
        );

        matches!(verifier_result, crate::outcome_verifier::CompletionStatus::Approved)
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionCondition, SubgoalCompletionEngine};

    #[test]
    fn host_observation_directive_creates_exact_command_contract() {
        let directive = "Perform a read-only investigation and report the current host uptime, filesystem capacity for /, available memory/RAM, and load average. Do not use SearchMemory.";

        let contract = SubgoalCompletionEngine::infer_contract("task_1", directive);

        assert_eq!(
            contract.conditions,
            vec![CompletionCondition::CommandExecutionVerified {
                command: "uptime; df -h /; free -m".to_string(),
            }]
        );
        assert!(!contract.is_mutation_task);
        assert!(contract.required_artifact_paths.is_empty());
    }

    #[test]
    fn explicit_search_memory_directive_creates_marker_only_contract() {
        let directive = "READ-ONLY MEMORY TEST. Do not modify, create, delete, or write any files. Do not execute shell commands. Use the SearchMemory tool to recall prior memory specifically about host observations involving uptime, filesystem or disk capacity, available memory/RAM, and load average. This is a semantic-memory recall test only. After SearchMemory returns, report exactly what memory was retrieved and whether it contained actual host-metric observations.";

        let contract = SubgoalCompletionEngine::infer_contract("task_1", directive);

        assert_eq!(contract.conditions, vec![CompletionCondition::SearchMemoryMarkerAcquired]);
        assert!(!contract.is_mutation_task);
        assert!(contract.required_artifact_paths.is_empty());
    }

    #[test]
    fn search_memory_evidence_satisfies_native_contract() {
        let directive = "Use the SearchMemory tool to recall prior host observations.";

        let contract = SubgoalCompletionEngine::infer_contract("task_1", directive);

        assert!(SubgoalCompletionEngine::evaluate(
            &contract,
            "[SEARCH_MEMORY_OK] Found semantic context: host_report.md",
            0.10,
        ));
    }
}

fn extract_required_artifact_paths(objective: &str) -> Vec<ArtifactSpec> {
    let mut out: Vec<ArtifactSpec> = Vec::new();
    let exts = ["md", "json", "rs", "toml", "yaml", "yml", "txt", "py"];
    let tokens: Vec<&str> = objective.split_whitespace().collect();
    let write_words =
        ["write", "save", "export", "synthesize", "create", "implement", "integrate", "add"];

    for (i, tok) in tokens.iter().enumerate() {
        // Completion criteria such as `WRITE_OK:host_report.md` are
        // evidence tokens, not artifact paths. Never allow a marker prefix
        // to contaminate ArtifactSpec.path.
        let trimmed = tok.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '.' && c != '/' && c != '_' && c != '-'
        });
        let clean = trimmed
            .strip_prefix("WRITE_OK:")
            .or_else(|| trimmed.strip_prefix("DIFF_PATCH_OK:"))
            .unwrap_or(trimmed);

        if exts.iter().any(|ext| clean.ends_with(&format!(".{}", ext)))
            && (clean.contains('/') || clean.contains('.'))
        {
            // Scan back to the start of this file mention's clause (comma boundary,
            // capped at 30 tokens) rather than a fixed 3-token window -- a "read ... in
            // X" clause can run long enough that the verb falls outside a small window.
            let min_start = i.saturating_sub(30);
            let mut start = i;
            while start > min_start && !tokens[start - 1].ends_with(',') {
                start -= 1;
            }

            // Match write-verbs as whole words, not substrings: a bare "to" is too
            // generic (it's the infinitive/preposition in "read ... to understand"),
            // and naive substring matching lets words like "tools" falsely match "to".
            let is_write_tgt = tokens[start..i].iter().any(|w| {
                let wl = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                write_words.contains(&wl.as_str())
            });

            // Only register a required write target on an explicit write-verb match in
            // this clause. An ambiguous mention with no write signal must NOT default to
            // "write" -- that risks overwriting a file that was meant to be read only.
            if is_write_tgt && !out.iter().any(|a| a.path == clean) {
                out.push(ArtifactSpec {
                    path: clean.to_string(),
                    reason: "output_deliverable".into(),
                });
            }
        }
    }

    out
}
