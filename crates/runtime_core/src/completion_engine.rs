use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompletionCondition {
    ReadMarkerAcquired,
    /// Path-bound read requirement: evidence must contain a [READ_OK] or
    /// [READ_CHUNK_OK] File: <path> marker whose recorded path matches
    /// `path` exactly.
    ReadPathAcquired {
        path: String,
    },
    SearchMarkerAcquired,
    /// Term-bound search requirement: evidence must contain a
    /// [SEARCH_CODE_OK] marker whose recorded query matches `term`
    /// (substring, case-insensitive -- mirrors the tolerance already
    /// given to action_matches_strategy's search_code: query check in
    /// cortical_loop.rs). Confirmed root cause of a mission-scoped
    /// SEARCH_CODE_OK:<path>:<term> criterion being satisfied by an
    /// unrelated inherited [SEARCH_CODE_OK] marker from a completely
    /// different search (e.g. via SearchMemory recall or DAG lineage),
    /// because the bare SearchMarkerAcquired this criterion previously
    /// collapsed into carries no term or path at all.
    SearchTermAcquired {
        term: String,
    },
    SearchMemoryMarkerAcquired,
    WriteMarkerAcquired,
    /// Path-bound write requirement: evidence must contain a [WRITE_OK]
    /// marker whose recorded path matches `path` exactly.
    WritePathAcquired {
        path: String,
    },
    PatchMarkerAcquired,
    CommandMarkerAcquired,
    CommandExecutionVerified {
        command: String,
    },
    LineageEvidenceAcquired,
    ReadFunctionMarkerAcquired {
        path: String,
        function_name: String,
    },
    SourceCodeEvidenceRequired,
    MinBeliefScore(f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgoalContract {
    pub node_id: String,
    pub objective: String,
    pub conditions: Vec<CompletionCondition>,
    pub is_mutation_task: bool,
    /// True when the mission declares read-only intent ("READ-ONLY",
    /// "read only", "audit only", or "do not modify any source files").
    /// When set, the cortical loop refuses every write/patch/synthesize
    /// action before the ActionGate is consulted, regardless of what
    /// later task text says. Absolute block: a read-only mission cannot
    /// mutate the substrate.
    #[serde(default)]
    pub read_only: bool,
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

/// Return true when completion requires a semantic conclusion rather than
/// merely proving that a physical/tool action occurred.
///
/// This is intentionally centralized because the same classification must
/// govern every terminal-completion shortcut in the cortical loop. Physical
/// evidence such as READ_OK, SEARCH_CODE_OK, or HALT is not itself proof that
/// an analysis objective was satisfied.
pub(crate) fn requires_semantic_verification(objective: &str) -> bool {
    let mut text = objective.trim().to_lowercase();

    if let Some(stripped) = text.strip_prefix("objective:") {
        text = stripped.trim().to_string();
    } else if let Some(stripped) = text.strip_prefix("objective") {
        text = stripped.trim().to_string();
    }

    // An explicitly leading command is a physical execution objective even
    // when the command itself contains semantic vocabulary such as
    // "check", "verify", or "compute" (for example, "Execute cargo check").
    // This mirrors infer_contract()'s command-objective precedence.
    let is_explicit_command =
        text.strip_prefix("execute ").is_some()
            || text.strip_prefix("run ").is_some();

    if is_explicit_command && extract_required_command(&text).is_some() {
        return false;
    }

    any_word_unnegated(
        &text,
        &[
            "analyze",
            "analyse",
            "analysis",
            "investigate",
            "investigation",
            "audit",
            "compare",
            "comparison",
            "synthesize",
            "synthesis",
            "verify",
            "compute",
            "check",
            "derive",
            "determine",
            "report",
            "reporting",
        ],
    )
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

pub(crate) fn extract_required_command(objective: &str) -> Option<String> {
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

    // Accept unquoted executable commands in natural-language directives,
    // e.g. "Execute cargo check -p math_utils to verify ...".
    const KNOWN_BINARIES: &[&str] = &[
        "cargo", "git", "ls", "find", "cat", "echo", "cd", "rm", "cp", "mv", "mkdir", "touch",
        "grep", "rg", "python", "python3", "node", "npm", "yarn", "rustc", "make", "docker",
        "bash", "sh",
    ];

    const STOPPERS: &[&str] = &[" to ", " and ", " so ", ". ", " that ", " which ", ", "];

    for verb in ["execute ", "run "] {
        let mut search_from = 0usize;

        while let Some(rel) = lower[search_from..].find(verb) {
            let verb_end = search_from + rel + verb.len();
            let tail = &objective[verb_end..];
            let line = tail.lines().next().unwrap_or("");
            let first_word = line.split_whitespace().next().unwrap_or("");

            if KNOWN_BINARIES.iter().any(|binary| first_word.eq_ignore_ascii_case(binary)) {
                let mut command = line.to_string();

                for stopper in STOPPERS {
                    if let Some(pos) = command.find(stopper) {
                        command.truncate(pos);
                    }
                }

                let command = command.trim().trim_end_matches('.').trim();

                if !command.is_empty() {
                    return Some(command.to_string());
                }
            }

            search_from = verb_end;
        }
    }

    None
}

/// Collect every target named on a `<marker><target>` line of a mission.
///
/// Unlike `extract_named_target` (single first-match), this scans the
/// entire objective, deduplicates, and returns targets in order of
/// appearance.
pub(crate) fn extract_all_named_targets(objective: &str, marker: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = objective[search_from..].find(marker) {
        let pos = search_from + rel;
        let remainder = &objective[pos + marker.len()..];
        if let Some(line) = remainder.lines().next() {
            let target = line.trim().trim_matches('`').trim_matches('"');
            if !target.is_empty() && !out.iter().any(|t| t == target) {
                out.push(target.to_string());
            }
        }
        search_from = pos + marker.len();
    }
    out
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

        // READ-ONLY INTENT DETECTION.
        //
        // When the mission declares read-only intent, set this flag on the
        // contract. The cortical loop's handle_execute refuses every write,
        // patch, and synthesize action when this flag is set, before the
        // ActionGate is consulted. This is an absolute block: it does not
        // depend on the strategist's proposal, the critic's approval, or
        // the planner's DAG shape.
        //
        // The heuristic matches the declaration forms used by real
        // missions. Matching is on the already-lowercased objective text,
        // so case is irrelevant. See also cortical_loop::handle_execute.
        let read_only = classify_text.contains("read-only")
            || classify_text.contains("read only")
            || classify_text.contains("audit only")
            || classify_text.contains("do not modify any source files");

        // Explicit PATCH_FUNCTION_OK contract.
        //
        // Guarded by !read_only so a READ-ONLY mission whose text also
        // mentions patch_function cannot produce a mutation condition.
        // Without this, the contract would carry PatchMarkerAcquired
        // alongside read_only = true, and the invariant "read-only never
        // mutates" would rely solely on the execute-time refusal in
        // cortical_loop::handle_execute rather than being true by
        // construction.
        if (objective.contains("PATCH_FUNCTION_OK:")
            || (classify_text.contains("patch_function")
                && !classify_text.contains("read_function")))
            && !read_only
        {
            return SubgoalContract {
                node_id: node_id.to_string(),
                objective: objective.to_string(),
                conditions: vec![CompletionCondition::PatchMarkerAcquired],
                is_mutation_task: true,
                read_only,
                required_artifact_paths: Vec::new(),
            };
        }

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
                    read_only,
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
                read_only,
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
                read_only,
                required_artifact_paths: Vec::new(),
            };
        }

        // A directive that LEADS with an explicit command verb
        // ("Execute ...", "Run ...") followed by a recognized binary is a
        // command objective, not a mutation objective, even when its
        // verification clause references a previously-applied "patch" as
        // a noun. This mirrors the precedence of `generate_hypotheses`,
        // which checks `extract_required_command` before any patch/write
        // intent detection. Without this check, `infer_contract` would
        // classify "Execute cargo check -p foo to verify the patch
        // compiles" as a mutation task and require PatchMarkerAcquired,
        // which command execution can never satisfy.
        //
        // The guard is intentionally narrow: only apply when the
        // directive *begins* with the command verb, so that a compound
        // directive like "Patch X and then run cargo check" is not
        // silently downgraded to a command-only contract that discards
        // the patch requirement.
        let leads_with_command_verb =
            classify_text.starts_with("execute ") || classify_text.starts_with("run ");
        if leads_with_command_verb {
            if let Some(command) = extract_required_command(objective) {
                return SubgoalContract {
                    node_id: node_id.to_string(),
                    objective: objective.to_string(),
                    conditions: vec![CompletionCondition::CommandExecutionVerified { command }],
                    is_mutation_task: false,
                    read_only,
                    required_artifact_paths: Vec::new(),
                };
            }
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
        // SearchMemory is authoritative when explicitly requested -- BUT
        // when the mission ALSO names path-bound criteria (READ_OK:,
        // WRITE_OK:), do not discard them. Fall through so the intent
        // block below preserves every declared criterion.
        let has_explicit_path_bound =
            objective.contains("READ_OK:") || objective.contains("WRITE_OK:");
        if has_search_memory_intent && !has_explicit_path_bound {
            return SubgoalContract {
                node_id: node_id.to_string(),
                objective: objective.to_string(),
                conditions: vec![CompletionCondition::SearchMemoryMarkerAcquired],
                is_mutation_task: false,
                read_only,
                required_artifact_paths: Vec::new(),
            };
        }

        // Detect explicit path-bound criteria. When present, they are
        // authoritative for their respective category: emit one typed
        // condition per named target instead of the path-blind default.
        // Every OTHER declared criterion is preserved.
        let read_targets = extract_all_named_targets(objective, "READ_OK:");
        let write_targets = extract_all_named_targets(objective, "WRITE_OK:");
        let search_code_terms = extract_all_named_targets(objective, "SEARCH_CODE_OK:");
        let search_memory_terms = extract_all_named_targets(objective, "SEARCH_MEMORY_OK:");
        let has_path_bound_reads = !read_targets.is_empty();
        let has_path_bound_writes = !write_targets.is_empty();
        let has_search_code_marker = !search_code_terms.is_empty();
        let has_search_memory_marker = !search_memory_terms.is_empty();

        if has_patch && !read_only {
            is_mutation_task = true;
            conditions.push(CompletionCondition::PatchMarkerAcquired);
        }

        if (has_write || has_path_bound_writes) && !read_only {
            is_mutation_task = true;
            if has_path_bound_writes {
                for path in &write_targets {
                    conditions.push(CompletionCondition::WritePathAcquired { path: path.clone() });
                }
            } else {
                conditions.push(CompletionCondition::WriteMarkerAcquired);
            }
        }

        if has_path_bound_reads {
            for path in &read_targets {
                conditions.push(CompletionCondition::ReadPathAcquired { path: path.clone() });
            }
        } else if has_read_intent && !has_search_code_marker {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
        }

        if has_search_code_marker {
            for term in &search_code_terms {
                conditions.push(CompletionCondition::SearchTermAcquired { term: term.clone() });
            }
        } else if has_search_intent {
            conditions.push(CompletionCondition::SearchMarkerAcquired);
        }

        if has_search_memory_intent || has_search_memory_marker {
            conditions.push(CompletionCondition::SearchMemoryMarkerAcquired);
        }

        // Analysis/investigation over source code requires actual source
        // evidence. This is deliberately independent of WRITE_OK.
        // SourceCodeEvidenceRequired is skipped ONLY when path-bound reads
        // exist. Named reads are strictly stronger than "any source file".
        // Skip when a stronger SEARCH_CODE_OK: criterion already exists.
        if (has_analysis_intent || (has_read_intent && has_search_intent))
            && !has_path_bound_reads
            && !has_search_code_marker
        {
            conditions.push(CompletionCondition::SourceCodeEvidenceRequired);
        }

        // Recon-only objectives still need a belief threshold. A mission
        // that names specific READ_OK: targets counts as recon: the named
        // reads are the reconnaissance contract.
        let has_recon = has_read_intent
            || has_search_intent
            || has_analysis_intent
            || has_path_bound_reads
            || has_search_memory_marker;

        // A contract whose substantive conditions are exclusively search
        // markers does not need a belief threshold. The search marker is
        // itself the physical evidence; the threshold requires an
        // additional level of cognitive confidence that a search
        // producing its matches does not, by itself, generate. That has
        // blocked completion of search-only nodes whose physical criteria
        // were already satisfied -- the post-search expansion in the
        // cortical loop would inject read_file hypotheses to raise
        // belief, and the injected strategy would pin the node in a loop
        // instead of completing on the search result it already had.
        //
        // A belief threshold is still added when the contract requires
        // reads, writes, patches, or analysis: in those cases belief is
        // a proxy for "the cortex explored enough" before mutating the
        // substrate or committing to an analysis conclusion.
        let only_search_markers = !conditions.is_empty()
            && conditions.iter().all(|c| {
                matches!(
                    c,
                    CompletionCondition::SearchTermAcquired { .. }
                        | CompletionCondition::SearchMarkerAcquired
                        | CompletionCondition::SearchMemoryMarkerAcquired
                )
            });

        if has_recon {
            if !only_search_markers {
                let threshold =
                    if has_write || has_patch || has_path_bound_writes { 0.75 } else { 0.65 };
                conditions.push(CompletionCondition::MinBeliefScore(threshold));
            }
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

        // When the mission declares read-only intent, no artifact can be
        // produced -- handle_execute refuses every write/patch/synthesize
        // action -- so required_artifact_paths must be empty. Otherwise
        // ContractEvaluator would demand a [WRITE_OK] marker for a path
        // that can never be written, and the node would never satisfy its
        // contract.
        let arts = if read_only { Vec::new() } else { extract_required_artifact_paths(objective) };
        if !read_only
            && !arts.is_empty()
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
            read_only,
            required_artifact_paths: arts,
        }
    }

    pub fn evaluate(contract: &SubgoalContract, local_evidence: &str, belief_score: f32) -> bool {
        crate::outcome_verifier::OutcomeVerifier::mission_completion_authorized(
            local_evidence,
            belief_score,
            &contract.conditions,
        )
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
                        | CompletionCondition::WritePathAcquired { .. }
                )
            })
            .cloned()
            .collect();

        crate::outcome_verifier::OutcomeVerifier::mission_completion_authorized(
            local_evidence,
            belief_score,
            &recon_conditions,
        )
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
        // A token that carried a WRITE_OK: / DIFF_PATCH_OK: prefix is
        // itself a write declaration. The prefix is authoritative; the
        // scan-back for a write verb is only needed for unmarked file
        // mentions. Without this, a `WRITE_OK:<path>` under a
        // "MANDATORY NODE SUCCESS CRITERIA:" header had no adjacent
        // write verb, was silently ignored, and left the mission
        // contract with no required artifacts -- so write_phase_ready
        // never became true and every write proposal was rejected as a
        // STRATEGY VIOLATION against the reconnaissance strategy.
        let was_criterion_marked =
            trimmed.starts_with("WRITE_OK:") || trimmed.starts_with("DIFF_PATCH_OK:");

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
            let is_write_tgt = was_criterion_marked
                || tokens[start..i].iter().any(|w| {
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

#[cfg(test)]
mod tests {
    use super::{requires_semantic_verification, CompletionCondition, SubgoalCompletionEngine};

    #[test]
    fn semantic_verification_classifier_covers_analysis_objectives() {
        let semantic_objectives = [
            "Analyze the implementation",
            "Investigate the failure",
            "Perform a final audit",
            "Compare the two implementations",
            "Synthesize the findings",
            "Verify the stated invariant",
            "Compute the resulting value",
            "Check the contract",
            "Derive the conclusion",
        ];

        for objective in semantic_objectives {
            assert!(
                super::requires_semantic_verification(objective),
                "semantic objective was not classified: {objective}"
            );
        }
    }

    #[test]
    fn semantic_verification_classifier_covers_host_investigation_and_reporting() {
        let semantic_objectives = [
            "Perform a read-only investigation and report the current host uptime, filesystem capacity for /, available memory/RAM, and load average.",
            "Determine the current host uptime and report the findings.",
            "Perform an investigation and report the results.",
        ];

        for objective in semantic_objectives {
            assert!(
                requires_semantic_verification(objective),
                "Expected semantic verification for: {objective}"
            );
        }
    }

    #[test]
    fn semantic_verification_classifier_rejects_physical_only_objectives() {
        let physical_objectives = [
            "Read crates/runtime_core/src/lib.rs",
            "Search for SEARCH_CODE_OK:foo",
            "Execute cargo check -p nexus_runtime_core",
            "Run uptime; df -h /; free -m",
        ];

        for objective in physical_objectives {
            assert!(
                !super::requires_semantic_verification(objective),
                "physical-only objective was incorrectly classified as semantic: {objective}"
            );
        }
    }

    #[test]
    fn read_only_mission_sets_flag() {
        let mission = "Perform a READ-ONLY final audit of the compiler-remediation implementation.";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(contract.read_only, "READ-ONLY mission must set read_only");
        assert!(
            !contract.is_mutation_task,
            "READ-ONLY mission must not be classified as a mutation task"
        );
    }

    #[test]
    fn read_only_audit_only_phrase_sets_flag() {
        let mission = "Audit only. Do NOT modify any source files.";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(contract.read_only);
    }

    #[test]
    fn read_only_suppresses_write_even_with_synthesize_language() {
        // The exact shape that caused the 2026-09-14 audit-run report
        // write: a READ-ONLY mission whose DAG node objective also says
        // "synthesize the final report". The read-only declaration wins;
        // no write condition may be pushed and no required artifact may
        // be retained.
        let mission = "Perform a comprehensive READ-ONLY final audit.                        In section 3 synthesize the final report.";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(contract.read_only, "READ-ONLY must be detected");
        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::WriteMarkerAcquired
                    | CompletionCondition::WritePathAcquired { .. }
                    | CompletionCondition::PatchMarkerAcquired
            )),
            "read-only mission must not require any write or patch condition"
        );
        assert!(
            contract.required_artifact_paths.is_empty(),
            "read-only mission must not carry required artifact paths"
        );
    }

    #[test]
    fn read_only_patch_mission_does_not_produce_mutation_condition() {
        let mission = "READ-ONLY audit. Do not modify source. Apply a patch_function to fix any issues found.";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(contract.read_only);
        assert!(!contract.is_mutation_task);

        assert!(!contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::PatchMarkerAcquired
                | CompletionCondition::WriteMarkerAcquired
                | CompletionCondition::WritePathAcquired { .. }
        )));
    }

    #[test]
    fn non_read_only_mission_preserves_write_contract() {
        let mission = "Write model_routing_analysis.md documenting the results.";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(!contract.read_only, "non-read-only mission must not set the flag");
    }

    #[test]
    fn host_observation_directive_preserves_command_contract_but_requires_semantic_verification() {
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
        assert!(requires_semantic_verification(directive));
    }

    #[test]
    fn explicit_search_memory_directive_creates_marker_only_contract() {
        let directive = "READ-ONLY MEMORY TEST. Do not modify, create, delete, or write any files. Do not execute shell commands. Use the SearchMemory tool to recall prior memory specifically about host observations involving uptime, filesystem or disk capacity, available memory/RAM, and load average. This is a semantic-memory recall test only. After SearchMemory returns, report exactly what memory was retrieved and whether it contained actual host-metric observations.";

        let contract = SubgoalCompletionEngine::infer_contract("task_1", directive);

        assert_eq!(contract.conditions, vec![CompletionCondition::SearchMemoryMarkerAcquired]);
        assert!(!contract.is_mutation_task);
        assert!(contract.required_artifact_paths.is_empty());
    }

    // ------------------------------------------------------------------
    // Path-bound READ_OK / WRITE_OK contract coverage
    // ------------------------------------------------------------------

    #[test]
    fn case_1_twelve_reads_produce_twelve_conditions() {
        let mission = "READ_OK:file1.rs\nREAD_OK:file2.rs\nREAD_OK:file3.rs\nREAD_OK:file4.rs\nREAD_OK:file5.rs\nREAD_OK:file6.rs\nREAD_OK:file7.rs\nREAD_OK:file8.rs\nREAD_OK:file9.rs\nREAD_OK:file10.rs\nREAD_OK:file11.rs\nREAD_OK:file12.rs";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        let paths: Vec<&str> = contract
            .conditions
            .iter()
            .filter_map(|c| match c {
                CompletionCondition::ReadPathAcquired { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 12);
        assert_eq!(paths[0], "file1.rs");
        assert_eq!(paths[11], "file12.rs");
    }

    #[test]
    fn case_2_reads_plus_search_code_preserve_both() {
        let mission = "READ_OK:a.rs\nREAD_OK:b.rs\nSEARCH_CODE_OK:compiler_critic";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert_eq!(
            contract
                .conditions
                .iter()
                .filter(|c| matches!(c, CompletionCondition::ReadPathAcquired { .. }))
                .count(),
            2
        );
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::SearchTermAcquired { term } if term == "compiler_critic"
        )));
    }

    #[test]
    fn case_3_reads_plus_search_memory_preserve_both() {
        let mission = "READ_OK:a.rs\nSEARCH_MEMORY_OK:prior_lessons";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(contract.conditions.iter().any(
            |c| matches!(c, CompletionCondition::ReadPathAcquired { path } if path == "a.rs")
        ));
        assert!(contract
            .conditions
            .iter()
            .any(|c| matches!(c, CompletionCondition::SearchMemoryMarkerAcquired)));
    }

    #[test]
    fn case_4_reads_plus_all_criteria_preserve_everything() {
        let mission = "READ_OK:a.rs\nREAD_OK:b.rs\nSEARCH_CODE_OK:foo\nSEARCH_MEMORY_OK:bar";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert_eq!(
            contract
                .conditions
                .iter()
                .filter(|c| matches!(c, CompletionCondition::ReadPathAcquired { .. }))
                .count(),
            2
        );
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::SearchTermAcquired { term } if term == "foo"
        )));
        assert!(contract
            .conditions
            .iter()
            .any(|c| matches!(c, CompletionCondition::SearchMemoryMarkerAcquired)));
    }

    #[test]
    fn case_5_path_bound_reads_exclude_source_code_evidence_required() {
        let mission = "Perform a READ-ONLY audit.\nREAD_OK:a.rs\nREAD_OK:b.rs";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(
            !contract
                .conditions
                .iter()
                .any(|c| matches!(c, CompletionCondition::SourceCodeEvidenceRequired)),
            "SourceCodeEvidenceRequired must be omitted when path-bound reads exist"
        );
    }

    #[test]
    fn case_6_path_bound_reads_keep_belief_threshold() {
        let mission = "READ_OK:a.rs\nREAD_OK:b.rs";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(contract
            .conditions
            .iter()
            .any(|c| matches!(c, CompletionCondition::MinBeliefScore(_))));
    }

    #[test]
    fn search_contract_with_analysis_mission_skips_belief_threshold() {
        // Reproduces the OpenFang workspace-node failure: a mission whose
        // objective uses analysis words ("inspect", "determine") and
        // whose criteria block names a single SEARCH_CODE_OK: target.
        // Previously the analysis words caused has_recon to fire and the
        // contract acquired MinBeliefScore(0.65) on top of the search
        // marker. The search succeeded at low belief and the threshold
        // blocked completion; the post-search expansion then pinned the
        // node to an unrelated read_file strategy.
        let mission = "Inspect the workspace configuration under vendor/openfang, \
                      determining the crate count and structure.\n\n\
                      MANDATORY NODE SUCCESS CRITERIA:\n\
                      SEARCH_CODE_OK:vendor/openfang:Cargo.toml";
        let contract = SubgoalCompletionEngine::infer_contract("node_workspace", mission);

        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::SearchTermAcquired { term }
                if term == "vendor/openfang:Cargo.toml"
        )));

        assert!(
            !contract
                .conditions
                .iter()
                .any(|c| matches!(c, CompletionCondition::MinBeliefScore(_))),
            "analysis-shaped search-only contract must not carry a belief threshold; conditions = {:?}",
            contract.conditions
        );

        let evidence = "[SEARCH_CODE_OK] Path: vendor/openfang | Query: 'Cargo.toml'\nMATCHES:\nworkspace member listing\n";
        assert!(SubgoalCompletionEngine::reconnaissance_complete(&contract, evidence, 0.10,));
    }

    #[test]
    fn case_7_search_only_contract_requires_search_marker_only() {
        let mission = "SEARCH_CODE_OK:some_term";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        // The search criterion is present.
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::SearchTermAcquired { term } if term == "some_term"
        )));

        // A search-only contract carries no belief threshold. The search
        // marker is the entire physical contract; requiring a confidence
        // gate above it has blocked completion of nodes whose criteria
        // were already satisfied.
        assert!(
            !contract
                .conditions
                .iter()
                .any(|c| matches!(c, CompletionCondition::MinBeliefScore(_))),
            "search-only contract must not carry a belief threshold; conditions = {:?}",
            contract.conditions
        );

        // Empty evidence does not satisfy the marker.
        assert!(!SubgoalCompletionEngine::reconnaissance_complete(&contract, "", 0.9));

        // Matching evidence satisfies the contract at any belief.
        let search_evidence = "[SEARCH_CODE_OK] Path: src | Query: some_term\nMATCHES:\nline 1\n";
        assert!(SubgoalCompletionEngine::reconnaissance_complete(&contract, search_evidence, 0.0));
        assert!(SubgoalCompletionEngine::reconnaissance_complete(&contract, search_evidence, 0.9));
    }

    #[test]
    fn path_bound_reads_do_not_discard_patch_marker() {
        let mission = "PATCH_FUNCTION_OK:a.rs:foo\nREAD_OK:b.rs";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(contract
            .conditions
            .iter()
            .any(|c| matches!(c, CompletionCondition::PatchMarkerAcquired)));
    }

    #[test]
    fn leading_command_directive_with_patch_noun_gets_command_contract() {
        let directive = "Execute cargo check -p math_utils to verify that the AST patch compiles cleanly with zero errors.";
        let contract = SubgoalCompletionEngine::infer_contract("task_3", directive);

        assert_eq!(
            contract.conditions,
            vec![CompletionCondition::CommandExecutionVerified {
                command: "cargo check -p math_utils".to_string(),
            }]
        );
        assert!(!contract.is_mutation_task);
        assert!(contract.required_artifact_paths.is_empty());
    }

    #[test]
    fn unquoted_cargo_command_extraction() {
        let directive =
            "Execute cargo check -p math_utils to verify that the AST patch compiles cleanly with zero errors.";

        let command = super::extract_required_command(directive);

        assert_eq!(command.as_deref(), Some("cargo check -p math_utils"));
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
