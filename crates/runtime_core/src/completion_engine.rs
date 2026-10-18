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
    /// Mathematical derivation execution has a deliberately broader
    /// evidence compatibility rule than ordinary command execution.
    /// This must remain explicit so mathematical evidence can never
    /// satisfy an unrelated CommandExecutionVerified contract.
    MathematicalExecutionVerified {
        command: String,
    },
    LineageEvidenceAcquired,
    ReadFunctionMarkerAcquired {
        path: String,
        function_name: String,
    },
    SourceCodeEvidenceRequired,
    /// Inline-analysis nodes: the deliverable is the HALT answer itself.
    HaltDelivered,
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

/// For mathematical-derivation nodes the planner embeds the full ALL/ANY
/// tables inside the command string.  After a successful execution the
/// evidence contains those same tables.  We treat the contract as satisfied
/// when the evidence shows the tables, even if the exact quoting of the
/// python3 -c string differs slightly (anti-loop / strategist variants).
pub(crate) fn math_command_satisfied(evidence: &str) -> bool {
    evidence.contains("ALL TABLE")
        && evidence.contains("ANY TABLE")
        && evidence.contains("ALL_COMMUTATIVE")
        && evidence.contains("DUALITY_ALL_36")
        && (evidence.contains("[CMD_OK]") || evidence.contains("Status: exit status: 0"))
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
/// Return the single Rust source path named by a structural-analysis
/// objective. Structural classification is intentionally narrow: this is
/// only an acquisition-shape helper and does not authorize completion.
pub(crate) fn structural_analysis_target(objective: &str) -> Option<String> {
    let lower = objective.to_lowercase();

    let structural = lower.contains("function structure")
        || lower.contains("function inventory")
        || lower.contains("large-function")
        || lower.contains("large function")
        || lower.contains("module structure")
        || lower.contains("call graph")
        || lower.contains("structural")
        || lower.contains("architecture");

    let analysis = lower.contains("audit")
        || lower.contains("analy")
        || lower.contains("inspect")
        || lower.contains("review")
        || lower.contains("summar");

    if !structural || !analysis {
        return None;
    }

    let mut paths = objective
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| "`'\"()[]{}.,:;".contains(c)))
        .filter(|token| token.ends_with(".rs") && token.contains('/'))
        .map(str::to_string)
        .collect::<Vec<_>>();

    paths.sort();
    paths.dedup();

    if paths.len() == 1 {
        paths.pop()
    } else {
        None
    }
}

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
        text.strip_prefix("execute ").is_some() || text.strip_prefix("run ").is_some();

    if is_explicit_command && extract_required_command(&text).is_some() {
        return false;
    }

    any_word_unnegated(
        &text,
        &[
            // --- additional base forms not previously covered ---
            "review",
            "explain",
            "assess",
            "evaluate",
            "interpret",
            "reason",
            "critique",
            "examine",
            "summarize",
            // --- present-participle forms ---
            //
            // LLM-planned objectives are frequently phrased in the present
            // participle ("Analyzing the call graph", "Investigating the
            // failure mode"), while this list is canonicalized to base
            // forms. The list itself is deliberately kept explicit rather
            // than teaching `has_word_unnegated` to strip suffixes: that
            // helper is shared with `infer_contract` and the planner, and
            // widening it would change write/read/search intent detection
            // globally. Only the semantic-objective classifier needs the
            // verb-morphology tolerance.
            "analyzing",
            "investigating",
            "reviewing",
            "evaluating",
            "assessing",
            "auditing",
            "comparing",
            "synthesizing",
            "verifying",
            "computing",
            "checking",
            "deriving",
            "determining",
            "reporting",
            "examining",
            "interpreting",
            "critiquing",
            "reasoning",
            "summarizing",
            // --- existing base forms ---
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
    // ASCII lowercase keeps byte offsets identical to `objective`.
    let lower = objective.to_ascii_lowercase();

    // Prefer an explicit command introduced by the standalone word
    // "command". Occurrences embedded in identifiers (`execute_command`)
    // name a tool, not the command to run, and are skipped.
    for (command_pos, _) in lower.match_indices("command") {
        let end = command_pos + "command".len();
        let boundary_before = lower[..command_pos]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let boundary_after = lower[end..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        if !(boundary_before && boundary_after) {
            continue;
        }
        let after = &objective[end..];

        // 1. Backtick-delimited command.
        if let Some(start) = after.find('`') {
            let remainder = &after[start + 1..];
            if let Some(close) = remainder.find('`') {
                let command = remainder[..close].trim();
                if !command.is_empty() {
                    return Some(command.to_string());
                }
            }
        }

        // 2. Quoted command immediately after the word (`command "uptime"`,
        //    `command: 'uptime'`). A quote deeper in the text belongs to the
        //    command's own payload and must not be taken as the command.
        let lead = after.trim_start().trim_start_matches(':').trim_start();
        for quote in ['"', '\''] {
            if let Some(remainder) = lead.strip_prefix(quote) {
                if let Some(close) = remainder.find(quote) {
                    let command = remainder[..close].trim();
                    if !command.is_empty() {
                        return Some(command.to_string());
                    }
                }
            }
        }

        // 3. Unquoted command after a colon on the same line, e.g.
        //    "run exactly this command: echo 'x' > f". Taken verbatim to end
        //    of line. Trailing prose would make evidence matching fail closed.
        let line = after.lines().next().unwrap_or("");
        if let Some(colon) = line.find(':') {
            let command = line[colon + 1..].trim();
            let starts_like_command = command
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '/' | '~'));
            if starts_like_command {
                return Some(command.to_string());
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

/// Structural scopes of a composed DAG-node mission.
pub(crate) struct MissionScopes<'a> {
    pub objective: &'a str,
    pub criteria: &'a str,
    pub root_directive: &'a str,
    #[allow(dead_code)]
    pub lineage: &'a str,
}

impl<'a> MissionScopes<'a> {
    pub fn parse(raw: &'a str) -> Self {
        const CRITERIA_HEADER: &str = "MANDATORY NODE SUCCESS CRITERIA:";
        const ROOT_HEADER: &str = "ROOT DIRECTIVE CONTEXT:";
        const LINEAGE_HEADER: &str = "LINEAGE CONTEXT:";

        let criteria_start = raw.find(CRITERIA_HEADER);
        let root_start = raw.find(ROOT_HEADER);
        let lineage_start = raw.find(LINEAGE_HEADER);

        if criteria_start.is_none() && root_start.is_none() {
            return Self { objective: raw, criteria: raw, root_directive: "", lineage: "" };
        }

        let objective_end = criteria_start.or(root_start).unwrap_or(raw.len());
        let objective = &raw[..objective_end];

        let criteria_end = root_start.or(lineage_start).unwrap_or(raw.len());
        let mut criteria = match criteria_start {
            Some(s) => &raw[s + CRITERIA_HEADER.len()..criteria_end],
            None => "",
        };
        if let Some(rule_start) = criteria.find("NODE COMPLETION RULE") {
            criteria = &criteria[..rule_start];
        }

        let root_directive = match (root_start, lineage_start) {
            (Some(rs), Some(ls)) if ls > rs => &raw[rs + ROOT_HEADER.len()..ls],
            (Some(rs), _) => &raw[rs + ROOT_HEADER.len()..],
            _ => "",
        };
        let lineage = match lineage_start {
            Some(ls) => &raw[ls + LINEAGE_HEADER.len()..],
            None => "",
        };

        Self { objective, criteria, root_directive, lineage }
    }

    pub fn inference_text(&self) -> String {
        if std::ptr::eq(self.objective, self.criteria) {
            return self.objective.to_string();
        }
        format!("{}\n{}", self.objective, self.criteria)
    }
}

fn detect_read_only(text: &str) -> bool {
    let lower = text.to_lowercase();
    const PHRASES: &[&str] = &[
        "read-only",
        "read only",
        "audit only",
        "do not modify any",
        "don't modify any",
        "without modifying any",
        "do not modify files",
        "don't modify files",
        "without modifying files",
        "do not modify, create",
        "do not modify/create",
        "do not write any",
        "don't write any",
        "without writing any",
        "do not create any",
        "don't create any",
        "do not change any",
        "don't change any",
        "no files may be modified",
        "must not be modified",
        "must not modify",
    ];
    PHRASES.iter().any(|p| lower.contains(p))
}

fn declared_criteria_contract(
    node_id: &str,
    text: &str,
    read_only: bool,
) -> Option<SubgoalContract> {
    let mut conditions: Vec<CompletionCondition> = Vec::new();
    let mut required_artifact_paths: Vec<ArtifactSpec> = Vec::new();

    for path in extract_all_named_targets(text, "READ_OK:") {
        conditions.push(CompletionCondition::ReadPathAcquired { path });
    }

    for path in extract_all_named_targets(text, "WRITE_OK:") {
        if read_only {
            continue;
        }
        conditions.push(CompletionCondition::WritePathAcquired { path: path.clone() });
        required_artifact_paths.push(ArtifactSpec { path, reason: "output_deliverable".into() });
    }

    if (text.contains("PATCH_FUNCTION_OK:")
        || text.contains("DIFF_PATCH_OK:")
        || text.contains("REPLACE_LINES_OK:"))
        && !read_only
    {
        conditions.push(CompletionCondition::PatchMarkerAcquired);
    }

    for term in extract_all_named_targets(text, "SEARCH_CODE_OK:") {
        conditions.push(CompletionCondition::SearchTermAcquired { term });
    }

    if text.contains("SEARCH_MEMORY_OK") {
        conditions.push(CompletionCondition::SearchMemoryMarkerAcquired);
    }

    if text.contains("DAG_LINEAGE_OK") || text.contains("LINEAGE MODE: INHERIT_ONLY") {
        conditions.push(CompletionCondition::LineageEvidenceAcquired);
    }

    if let Some(pos) = text.find("READ_FUNCTION_OK:") {
        let remainder = &text[pos + "READ_FUNCTION_OK:".len()..];
        let target =
            remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');
        let mut parts = target.splitn(2, ':');
        let path = parts.next().unwrap_or("").trim().to_string();
        let function_name = parts.next().unwrap_or("").trim().to_string();
        if !path.is_empty() && !function_name.is_empty() {
            conditions
                .push(CompletionCondition::ReadFunctionMarkerAcquired { path, function_name });
        }
    }

    if text.contains("CMD_OK") {
        if let Some(command) = extract_required_command(text) {
            conditions.push(CompletionCondition::CommandExecutionVerified { command });
        } else {
            conditions.push(CompletionCondition::CommandMarkerAcquired);
        }
    }

    if conditions.is_empty() {
        return None;
    }

    let only_search_markers = conditions.iter().all(|c| {
        matches!(
            c,
            CompletionCondition::SearchTermAcquired { .. }
                | CompletionCondition::SearchMarkerAcquired
                | CompletionCondition::SearchMemoryMarkerAcquired
        )
    });
    let has_output = conditions.iter().any(|c| {
        matches!(
            c,
            CompletionCondition::WritePathAcquired { .. }
                | CompletionCondition::WriteMarkerAcquired
                | CompletionCondition::PatchMarkerAcquired
        )
    });
    let has_recon = conditions.iter().any(|c| {
        matches!(
            c,
            CompletionCondition::ReadPathAcquired { .. }
                | CompletionCondition::ReadMarkerAcquired
                | CompletionCondition::ReadFunctionMarkerAcquired { .. }
                | CompletionCondition::SearchTermAcquired { .. }
                | CompletionCondition::SearchMarkerAcquired
                | CompletionCondition::SearchMemoryMarkerAcquired
                | CompletionCondition::SourceCodeEvidenceRequired
        )
    });

    if has_recon && !only_search_markers {
        let threshold = if has_output { 0.75 } else { 0.65 };
        conditions.push(CompletionCondition::MinBeliefScore(threshold));
    }

    Some(SubgoalContract {
        node_id: node_id.to_string(),
        objective: text.to_string(),
        is_mutation_task: has_output,
        read_only,
        conditions,
        required_artifact_paths,
    })
}

impl SubgoalCompletionEngine {
    pub fn infer_contract(node_id: &str, objective: &str) -> SubgoalContract {
        let scopes = MissionScopes::parse(objective);
        let inference_text = scopes.inference_text();
        let objective: &str = &inference_text;

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
        let declared_mutation_criteria = objective.contains("WRITE_OK:")
            || objective.contains("PATCH_FUNCTION_OK:")
            || objective.contains("DIFF_PATCH_OK:")
            || objective.contains("REPLACE_LINES_OK:");
        let read_only = !declared_mutation_criteria
            && (detect_read_only(objective) || detect_read_only(scopes.root_directive));

        if let Some(contract) = declared_criteria_contract(node_id, objective, read_only) {
            return contract;
        }

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

        // Inline-analysis contract: no reads, searches, or belief threshold.
        if objective.contains("ANALYSIS_OK")
            && !objective.contains("READ_OK:")
            && !objective.contains("SEARCH_CODE_OK:")
        {
            return SubgoalContract {
                node_id: node_id.to_string(),
                objective: objective.to_string(),
                conditions: vec![CompletionCondition::HaltDelivered],
                is_mutation_task: false,
                read_only: true,
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
        // The mathematical planner emits this explicit contract marker.
        // It is a protocol boundary, not inference from arbitrary evidence
        // strings such as "ALL TABLE".
        if objective.contains("MATHEMATICAL_EXECUTION_VERIFIED:") {
            if let Some(command) = extract_required_command(objective) {
                return SubgoalContract {
                    node_id: node_id.to_string(),
                    objective: objective.to_string(),
                    conditions: vec![CompletionCondition::MathematicalExecutionVerified {
                        command,
                    }],
                    is_mutation_task: false,
                    read_only,
                    required_artifact_paths: Vec::new(),
                };
            }
        }

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
            &["write", "export", "save", "implement", "integrate", "create", "synthesize"],
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
        let has_explicit_path_bound = objective.contains("READ_OK:")
            || objective.contains("WRITE_OK:")
            || objective.contains("SEARCH_CODE_OK:")
            || objective.contains("SEARCH_MEMORY_OK:")
            || objective.contains("PATCH_FUNCTION_OK:")
            || objective.contains("READ_FUNCTION_OK:");
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

        let structural_target = structural_analysis_target(objective);
        let structural_search_contract = structural_target.is_some()
            && !has_path_bound_reads
            && !has_search_code_marker
            && !has_patch
            && !has_write
            && !objective.contains("READ_FUNCTION_OK:");

        if has_path_bound_reads {
            for path in &read_targets {
                conditions.push(CompletionCondition::ReadPathAcquired { path: path.clone() });
            }
        } else if has_read_intent && !has_search_code_marker && !structural_search_contract {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
        }

        if structural_search_contract {
            let path = structural_target.as_ref().expect("structural target checked above");

            conditions
                .push(CompletionCondition::SearchTermAcquired { term: format!("{}:fn", path) });
        } else if has_search_code_marker {
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
            && !structural_search_contract
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

        if objective.contains("ANALYSIS_OK")
            && !conditions.iter().any(|c| matches!(c, CompletionCondition::HaltDelivered))
        {
            conditions.push(CompletionCondition::HaltDelivered);
        }

        crate::completion_engine::SubgoalContract {
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
    fn semantic_verification_classifier_requires_analysis_ok_semantic_gate() {
        assert!(
            requires_semantic_verification("ANALYSIS_OK"),
            "ANALYSIS_OK must require semantic verification"
        );
    }

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
    fn semantic_verification_classifier_coverage_table() {
        // Coverage table for `requires_semantic_verification`.
        //
        // The classifier gates every terminal completion shortcut in
        // `cortical_loop.rs` (five call sites as of this writing). A
        // false negative here is a silent completion-authority leak: a
        // semantic objective completes on a single physical marker
        // without ever passing through `verify_outcome`. A false
        // positive merely costs turns.
        //
        // Broader than `semantic_verification_classifier_covers_analysis_objectives`:
        // sweeps additional verbs, present-participle forms, and compound
        // objectives. Collects ALL mismatches into a Vec before asserting,
        // so one run reveals every gap in the classifier's word list.
        let should_trigger: &[(&str, &str)] = &[
            // Base-form verbs already asserted by the sibling test.
            ("Analyze the implementation", "base verb: analyze"),
            ("Investigate the failure", "base verb: investigate"),
            ("Perform a final audit", "base verb: audit"),
            ("Compare the two implementations", "base verb: compare"),
            ("Synthesize the findings", "base verb: synthesize"),
            ("Verify the stated invariant", "base verb: verify"),
            ("Compute the resulting value", "base verb: compute"),
            ("Check the contract", "base verb: check"),
            ("Derive the conclusion", "base verb: derive"),
            ("Determine the root cause", "base verb: determine"),
            ("Report the results", "base verb: report"),
            // Verbs/nouns frequently emitted by the LLM planner that are
            // NOT exercised by the sibling test.
            ("Review the implementation and explain the failure modes", "review / explain"),
            ("Assess the remediation strategy", "assess"),
            ("Evaluate the compiler output", "evaluate"),
            ("Summarize the audit findings", "summarize"),
            ("Interpret the error trace", "interpret"),
            ("Reason about the invariant", "reason"),
            ("Critique the proposal", "critique"),
            ("Examine the call graph", "examine"),
            // Present-participle forms. The classifier's token matcher
            // compares whole tokens case-insensitively; if the word list
            // contains only the base form, these will not match.
            ("Analyzing the call graph", "present participle: analyzing"),
            ("Investigating the failure mode", "present participle: investigating"),
            ("Reviewing the implementation", "present participle: reviewing"),
            ("Evaluating the compiler output", "present participle: evaluating"),
            ("Assessing the remediation strategy", "present participle: assessing"),
            // Compound objectives: a semantic word must still trigger even
            // when a physical verb appears earlier in the same objective.
            ("Read the file and analyze the implementation", "compound: read + analyze"),
            ("Search for the symbol, then summarize the findings", "compound: search + summarize"),
        ];

        // Objectives that MUST NOT be classified as semantic: purely
        // physical work whose completion is provable from a single marker.
        let should_not_trigger: &[&str] = &[
            "Read crates/runtime_core/src/lib.rs",
            "Search for SEARCH_CODE_OK:foo",
            "Execute cargo check -p nexus_runtime_core",
            "Run uptime; df -h /; free -m",
            "READ_OK:crates/runtime_core/src/router.rs",
            "SEARCH_CODE_OK:compiler_critic",
            "PATCH_FUNCTION_OK:crates/runtime_core/src/foo.rs:bar",
        ];

        let mut missing: Vec<(&str, &str)> = Vec::new();
        for (objective, category) in should_trigger {
            if !requires_semantic_verification(objective) {
                missing.push((objective, category));
            }
        }

        let mut false_positives: Vec<&str> = Vec::new();
        for objective in should_not_trigger {
            if requires_semantic_verification(objective) {
                false_positives.push(objective);
            }
        }

        assert!(
            missing.is_empty(),
            "classifier missed {} semantic objective(s) - add the missing \
             word(s) to the `any_word_unnegated` list in \
             `requires_semantic_verification`:\n  - {}",
            missing.len(),
            missing
                .iter()
                .map(|(obj, cat)| format!("[{cat}] {obj}"))
                .collect::<Vec<_>>()
                .join("\n  - ")
        );
        assert!(
            false_positives.is_empty(),
            "classifier false-positived {} physical objective(s) - \
             these must short-circuit before the word check:\n  - {}",
            false_positives.len(),
            false_positives.join("\n  - ")
        );
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
    fn declared_patch_criterion_overrides_objective_scope_restriction() {
        let mission = r#"OBJECTIVE: Add a doc comment immediately above foo without changing its behavior or signature. Do not change any other code.

MANDATORY NODE SUCCESS CRITERIA:
PATCH_FUNCTION_OK:crates/foo.rs:foo
"#;

        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(!contract.read_only);
        assert!(contract.is_mutation_task);
        assert!(contract
            .conditions
            .iter()
            .any(|c| matches!(c, CompletionCondition::PatchMarkerAcquired)));
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

    // Mission 2 shapes: the gate's two boundary cases, plus the belief
    // convention the widened disjunction depends on. Fixtures mirror the
    // existing SubgoalContract literals; only `conditions` varies.
    fn mk(conditions: Vec<CompletionCondition>) -> crate::completion_engine::SubgoalContract {
        crate::completion_engine::SubgoalContract {
            node_id: "test_node".to_string(),
            objective: "test objective".to_string(),
            conditions,
            is_mutation_task: false,
            read_only: false,
            required_artifact_paths: Vec::new(),
        }
    }

    fn composed_audit_mission(root_context: &str) -> String {
        format!(
            r#"OBJECTIVE: Perform a deterministic code audit of the provided Rust logic.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
- SEARCH_CODE_OK:crates/runtime_core/src:Evidence

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced.

ROOT DIRECTIVE CONTEXT:
{}

LINEAGE CONTEXT:
[NO COMPLETED OUTPUT AVAILABLE]"#,
            root_context
        )
    }

    #[test]
    fn root_context_output_labels_do_not_mint_mutation_conditions() {
        let mission = composed_audit_mission(
            r#"Then provide exactly this output format:
ROOT_CAUSE: <both independent defects>
TRACE: <...>
FIX: <the minimal logical correction>
Do not modify any files."#,
        );
        let contract = SubgoalCompletionEngine::infer_contract("task_1", &mission);

        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::PatchMarkerAcquired
                    | CompletionCondition::WriteMarkerAcquired
                    | CompletionCondition::WritePathAcquired { .. }
            )),
            "root context must not mint mutation conditions: {:?}",
            contract.conditions
        );
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::SearchTermAcquired { term }
                if term == "crates/runtime_core/src:Evidence"
        )));
        assert!(contract.read_only, "root-level 'Do not modify any files.' must set read_only");
    }

    #[test]
    fn failed_audit_run_contract_is_actually_satisfiable() {
        let mission = composed_audit_mission(
            r#"Provide the output format with a FIX: line.
Do not modify any files."#,
        );
        let contract = SubgoalCompletionEngine::infer_contract("task_1", &mission);

        let evidence = r#"[SEARCH_CODE_OK] Path: crates/runtime_core/src | Query: 'Evidence'
MATCHES:
lib.rs:1:enum Evidence
"#;
        assert!(
            SubgoalCompletionEngine::evaluate(&contract, evidence, 0.5),
            "conditions = {:?}",
            contract.conditions
        );
    }

    #[test]
    fn fix_label_in_objective_alongside_declared_criteria_is_inert() {
        let mission = r#"OBJECTIVE: Identify the defects and report FIX: the correction.

MANDATORY NODE SUCCESS CRITERIA:
SEARCH_CODE_OK:src:gate"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(
            !contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::PatchMarkerAcquired | CompletionCondition::WriteMarkerAcquired
            )),
            "conditions = {:?}",
            contract.conditions
        );
    }

    #[test]
    fn do_not_modify_any_files_sets_read_only() {
        let contract = SubgoalCompletionEngine::infer_contract(
            "t",
            "Audit the report. Do not modify any files.",
        );
        assert!(contract.read_only);
        assert!(contract.required_artifact_paths.is_empty());
    }

    #[test]
    fn lineage_output_cannot_impose_read_only_on_child_node() {
        let mission = r#"OBJECTIVE: Write summary.md of the findings.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
WRITE_OK:summary.md

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced.

ROOT DIRECTIVE CONTEXT:
Summarize the findings.

LINEAGE CONTEXT:
---
[DEPENDENCY LINEAGE FROM task_0]:
The upstream audit was read-only; do not modify any files.
---
"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(!contract.read_only, "lineage text must not impose read-only on the child");
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::WritePathAcquired { path } if path == "summary.md"
        )));
        assert_eq!(contract.required_artifact_paths.len(), 1);
    }

    #[test]
    fn wrapped_command_node_preserves_original_criteria() {
        let mission = r#"OBJECTIVE: Execute `cargo check -p math_utils` and report the actual command used and its output as verifiable evidence.

MANDATORY NODE SUCCESS CRITERIA:
CMD_OK

Then complete the original objective:
READ_OK:crates/runtime_core/src/lib.rs

ROOT DIRECTIVE CONTEXT:
Fix the build failure.

LINEAGE CONTEXT:
"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(
            contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::CommandExecutionVerified { command }
                    if command == "cargo check -p math_utils"
            )),
            "conditions = {:?}",
            contract.conditions
        );
        assert!(
            contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::ReadPathAcquired { path }
                    if path == "crates/runtime_core/src/lib.rs"
            )),
            "conditions = {:?}",
            contract.conditions
        );
    }

    #[test]
    fn mission_scopes_exclude_root_and_lineage() {
        let mission = composed_audit_mission("FIX: the correction.\nDo not modify any files.");
        let scopes = super::MissionScopes::parse(&mission);

        assert!(scopes.inference_text().contains("SEARCH_CODE_OK:"));
        assert!(!scopes.inference_text().contains("FIX:"), "leaked");
        assert!(scopes.root_directive.contains("FIX:"));
    }

    #[test]
    fn write_only_declared_contract_carries_no_belief_threshold() {
        let mission = r#"MANDATORY NODE SUCCESS CRITERIA:
WRITE_OK:out.md"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);
        assert!(!contract
            .conditions
            .iter()
            .any(|c| matches!(c, CompletionCondition::MinBeliefScore(_))));
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::WritePathAcquired { path } if path == "out.md"
        )));
    }

    #[test]
    fn declared_write_criterion_wins_over_root_read_only_restriction() {
        let mission = r#"OBJECTIVE: Produce the summary artifact.

MANDATORY NODE SUCCESS CRITERIA:
WRITE_OK:summary.md

ROOT DIRECTIVE CONTEXT:
Do not modify any files.
LINEAGE CONTEXT:
"#;
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(!contract.read_only);
        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::WritePathAcquired { path } if path == "summary.md"
        )));
    }

    #[test]
    fn belief_shortfall_reads_satisfied_at_the_gate() {
        // MinBeliefScore is the one condition no action can advance. Every
        // gate probe uses belief = 1.0, so it reads satisfied here and
        // proposals flow; the terminal gate passes the live belief, so
        // Complete still waits for the real propagation.
        let contract = mk(vec![
            CompletionCondition::CommandMarkerAcquired,
            CompletionCondition::MinBeliefScore(0.9),
        ]);
        let evidence = "[CMD_OK] Status: exit status: 0\n";
        assert!(SubgoalCompletionEngine::evaluate(&contract, evidence, 1.0));
    }

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
    fn case_8_search_code_plus_search_memory_without_reads_preserves_both() {
        // Regression: when SEARCH_CODE_OK: and SEARCH_MEMORY_OK: are both
        // present without READ_OK:/WRITE_OK:, infer_contract must not take
        // the SearchMemory early-return and silently discard SEARCH_CODE_OK:.
        let mission = "SEARCH_CODE_OK:foo\nSEARCH_MEMORY_OK:bar";
        let contract = SubgoalCompletionEngine::infer_contract("task_1", mission);

        assert!(
            contract.conditions.iter().any(|c| matches!(
                c,
                CompletionCondition::SearchTermAcquired { term } if term == "foo"
            )),
            "SEARCH_CODE_OK:foo must survive; conditions = {:?}",
            contract.conditions
        );

        assert!(
            contract
                .conditions
                .iter()
                .any(|c| matches!(c, CompletionCondition::SearchMemoryMarkerAcquired)),
            "SEARCH_MEMORY_OK must survive; conditions = {:?}",
            contract.conditions
        );
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
        // Reproduces the external repository workspace-node failure: a mission whose
        // objective uses analysis words ("inspect", "determine") and
        // whose criteria block names a single SEARCH_CODE_OK: target.
        // Previously the analysis words caused has_recon to fire and the
        // contract acquired MinBeliefScore(0.65) on top of the search
        // marker. The search succeeded at low belief and the threshold
        // blocked completion; the post-search expansion then pinned the
        // node to an unrelated read_file strategy.
        let mission = "Inspect the workspace configuration under vendor/external_repo, \
                      determining the crate count and structure.\n\n\
                      MANDATORY NODE SUCCESS CRITERIA:\n\
                      SEARCH_CODE_OK:vendor/external_repo:Cargo.toml";
        let contract = SubgoalCompletionEngine::infer_contract("node_workspace", mission);

        assert!(contract.conditions.iter().any(|c| matches!(
            c,
            CompletionCondition::SearchTermAcquired { term }
                if term == "vendor/external_repo:Cargo.toml"
        )));

        assert!(
            !contract
                .conditions
                .iter()
                .any(|c| matches!(c, CompletionCondition::MinBeliefScore(_))),
            "analysis-shaped search-only contract must not carry a belief threshold; conditions = {:?}",
            contract.conditions
        );

        let evidence = "[SEARCH_CODE_OK] Path: vendor/external_repo | Query: 'Cargo.toml'\nMATCHES:\nworkspace member listing\n";
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

// math_command_satisfied() is consumed only by the explicit
// MathematicalExecutionVerified contract in outcome_verifier.rs.
