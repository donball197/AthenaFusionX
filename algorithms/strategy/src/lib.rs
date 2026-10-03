//! Deterministic cognitive strategy algorithms.
//!
//! This crate deliberately has no dependency on runtime_core.
//! It decides WHAT strategy follows from an already-classified directive.
//! It does not execute actions, authorize mutations, select providers,
//! perform HTTP calls, or bypass ActionGate/completion verification.

/// Inputs supplied by the runtime after shared objective parsing.
///
/// The runtime owns the shared parsing primitives because they are also
/// consumed by completion/planner/task-manager code. This algorithm owns
/// the deterministic precedence decision.
#[derive(Debug, Clone, Default)]
pub struct DirectiveInputs {
    pub problem: String,
    pub problem_lower: String,

    pub has_explicit_success_criterion: bool,
    pub search_memory_explicit: bool,

    pub read_targets: Vec<String>,
    pub required_command: Option<String>,

    pub lineage_inherit_only: bool,
    pub host_observation: bool,
}

/// Deterministic directive-to-strategy compiler.
///
/// `Some(...)` means the directive contains enough deterministic information
/// that the probabilistic hypothesis generator should not guess a strategy.
/// `None` means the caller should fall through to probabilistic generation.
pub struct DirectiveStrategyCompiler;

impl DirectiveStrategyCompiler {
    pub fn compile(input: &DirectiveInputs) -> Option<Vec<String>> {
        let problem = input.problem.as_str();
        let problem_lower = input.problem_lower.as_str();

        // Explicit native SearchMemory directive.
        if input.search_memory_explicit {
            return Some(vec![format!("search_memory:{}", problem.trim())]);
        }

        // Explicit write/create directive precedence.
        if (problem_lower.contains("write_file")
            || problem_lower.contains("use write_file")
            || problem_lower.contains("create "))
            && !problem_lower.contains("read-only")
            && !problem_lower.contains("audit only")
        {
            for token in problem.split_whitespace() {
                let clean = token.trim_matches(|c: char| {
                    !c.is_alphanumeric() && c != '/' && c != '.' && c != '_'
                });

                if clean.contains('.')
                    && (clean.ends_with(".rs")
                        || clean.ends_with(".md")
                        || clean.ends_with(".json")
                        || clean.ends_with(".txt"))
                {
                    return Some(vec![format!("write_file:{}", clean)]);
                }
            }
        }

        // READ_FUNCTION_OK remains more specific than READ_OK.
        if let Some(pos) = problem.find("READ_FUNCTION_OK:") {
            let remainder = &problem[pos + "READ_FUNCTION_OK:".len()..];
            let target =
                remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');

            if !target.is_empty() && target.contains(':') {
                return Some(vec![format!("read_function:{}", target)]);
            }
        }

        // Deterministic READ_OK planning.
        if !input.read_targets.is_empty() {
            let mut hyps = Vec::new();

            if let Some(mem_pos) = problem.find("SEARCH_MEMORY_OK:") {
                let remainder = &problem[mem_pos + "SEARCH_MEMORY_OK:".len()..];
                let term = remainder
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('`')
                    .trim_matches('"');

                if !term.is_empty() {
                    hyps.push(format!("search_memory:{}", term));
                }
            }

            if let Some(search_pos) = problem.find("SEARCH_CODE_OK:") {
                let remainder = &problem[search_pos + "SEARCH_CODE_OK:".len()..];
                let query = remainder
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('`')
                    .trim_matches('"');

                if !query.is_empty() {
                    let search_path = std::path::Path::new(&input.read_targets[0])
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .filter(|p| !p.is_empty())
                        .unwrap_or_else(|| ".".to_string());

                    hyps.push(format!("search_code:{}:{}", search_path, query));
                }
            }

            for target in &input.read_targets {
                hyps.push(format!("read_file:{}", target));
            }

            return Some(hyps);
        }

        // Deterministic standalone SEARCH_CODE_OK.
        if let Some(pos) = problem.find("SEARCH_CODE_OK:") {
            let remainder = &problem[pos + "SEARCH_CODE_OK:".len()..];
            let line =
                remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');

            if !line.is_empty() {
                // Absolute path: /path/to/dir:term
                if let Some(stripped) = line.strip_prefix('/') {
                    if let Some(colon) = stripped.find(':') {
                        let search_path = format!("/{}", &stripped[..colon]);
                        let term = stripped[colon + 1..].trim();

                        if !term.is_empty() {
                            return Some(vec![format!("search_code:{}:{}", search_path, term)]);
                        }
                    }
                }

                // Workspace-relative path: vendor/openfang:Cargo.toml
                if let Some(colon) = line.rfind(':') {
                    let maybe_path = &line[..colon];
                    let maybe_term = line[colon + 1..].trim();

                    if maybe_path.contains('/') && !maybe_term.is_empty() {
                        return Some(vec![format!("search_code:{}:{}", maybe_path, maybe_term)]);
                    }
                }

                // Bare search term.
                return Some(vec![format!("search_code:crates/runtime_core/src:{}", line)]);
            }
        }

        // Deterministic standalone SEARCH_MEMORY_OK.
        if let Some(pos) = problem.find("SEARCH_MEMORY_OK:") {
            let remainder = &problem[pos + "SEARCH_MEMORY_OK:".len()..];
            let term =
                remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');

            if !term.is_empty() {
                return Some(vec![format!("search_memory:{}", term)]);
            }
        }

        // Deterministic standalone WRITE_OK.
        if let Some(pos) = problem.find("WRITE_OK:") {
            let remainder = &problem[pos + "WRITE_OK:".len()..];
            let target =
                remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');

            if !target.is_empty() {
                return Some(vec![format!("write_file:{}", target)]);
            }
        }

        // Imperative patch target takes precedence over verification command.
        if let Some((path, function)) = Self::extract_patch_target(problem, problem_lower) {
            return Some(Self::patch_strategies(&path, &function));
        }

        // Explicit command remains deterministic.
        if let Some(cmd) = &input.required_command {
            return Some(vec![format!("execute_command:{}", cmd)]);
        }

        // PATCH_FUNCTION_OK.
        if let Some(pos) = problem.find("PATCH_FUNCTION_OK:") {
            let remainder = &problem[pos + "PATCH_FUNCTION_OK:".len()..];
            let target =
                remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');

            if !target.is_empty() {
                if target.contains(':') {
                    return Some(Self::patch_strategies_from_target(target));
                }

                let path = problem
                    .split_whitespace()
                    .map(|tok| {
                        tok.trim_matches(|c: char| {
                            !c.is_alphanumeric() && c != '/' && c != '.' && c != '_'
                        })
                    })
                    .find(|tok| {
                        tok.contains('/')
                            && (tok.ends_with(".rs")
                                || tok.ends_with(".py")
                                || tok.ends_with(".js")
                                || tok.ends_with(".ts")
                                || tok.ends_with(".php"))
                    })
                    .unwrap_or("");

                if !path.is_empty() {
                    return Some(Self::patch_strategies(path, target));
                }
            }
        }

        // Legacy imperative patch_function fallback.
        if problem_lower.contains("patch_function") {
            let path = problem
                .split_whitespace()
                .map(|tok| {
                    tok.trim_matches(|c: char| {
                        !c.is_alphanumeric() && c != '/' && c != '.' && c != '_'
                    })
                })
                .find(|tok| {
                    tok.contains('/')
                        && (tok.ends_with(".rs")
                            || tok.ends_with(".py")
                            || tok.ends_with(".js")
                            || tok.ends_with(".ts")
                            || tok.ends_with(".php"))
                })
                .unwrap_or("");

            if !path.is_empty() {
                let words: Vec<&str> = problem.split_whitespace().collect();

                let mut func = "";

                for (i, word) in words.iter().enumerate() {
                    let wl = word.to_lowercase();

                    if (wl == "update" || wl == "function" || wl == "patch") && i + 1 < words.len()
                    {
                        let candidate =
                            words[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                        if !candidate.is_empty()
                            && candidate != "so"
                            && candidate != "to"
                            && candidate != "the"
                            && !candidate.contains('/')
                        {
                            func = candidate;
                            break;
                        }
                    }
                }

                if !func.is_empty() {
                    return Some(Self::patch_strategies(path, func));
                }
            }
        }

        // DIFF_PATCH_OK.
        if let Some(pos) = problem.find("DIFF_PATCH_OK:") {
            let remainder = &problem[pos + "DIFF_PATCH_OK:".len()..];
            let target =
                remainder.lines().next().unwrap_or("").trim().trim_matches('`').trim_matches('"');

            if !target.is_empty() {
                return Some(vec![format!("diff_patch:{}", target)]);
            }
        }

        // Lineage inheritance is a deterministic halt/report strategy.
        if input.lineage_inherit_only || problem_lower.contains("lineage mode: inherit_only") {
            return Some(vec![
                "halt:report inherited host observations from DAG lineage".to_string()
            ]);
        }

        // Host observations are deterministic physical operations.
        if input.host_observation {
            return Some(vec!["execute_command:uptime; df -h /; free -m".to_string()]);
        }

        None
    }

    /// B (multi-strategy patch targets): a patch target must never lock the
    /// cognition graph into a single hypothesis. Index 0 keeps the original
    /// patch_function strategy (and therefore the deterministic 0.51 turn-1
    /// tiebreak); indices 1.. are competing strategies that keep the node
    /// alive when the patch target proves unaddressable.
    fn patch_strategies(path: &str, function: &str) -> Vec<String> {
        vec![
            format!("patch_function:{}:{}", path, function),
            format!("read_file:{}", path),
            format!("replace_lines:{}", path),
        ]
    }

    /// Split a compound `path:function` target (PATCH_FUNCTION_OK form).
    fn patch_strategies_from_target(target: &str) -> Vec<String> {
        let mut parts = target.splitn(2, ':');
        let path = parts.next().unwrap_or("").trim();
        let function = parts.next().unwrap_or("").trim();

        if function.is_empty() {
            vec![format!("patch_function:{}", target)]
        } else {
            Self::patch_strategies(path, function)
        }
    }

    fn extract_patch_target(problem: &str, problem_lower: &str) -> Option<(String, String)> {
        let has_patch_verb = problem_lower.contains("patch_function")
            || problem_lower.contains("patch `")
            || problem_lower.contains("patch the")
            || problem_lower.contains("patch function");

        if !has_patch_verb {
            return None;
        }

        let path = problem
            .split_whitespace()
            .map(|tok| {
                tok.trim_matches(|c: char| {
                    !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
                })
            })
            .find(|tok| {
                tok.contains('/')
                    && (tok.ends_with(".rs")
                        || tok.ends_with(".py")
                        || tok.ends_with(".js")
                        || tok.ends_with(".ts")
                        || tok.ends_with(".php"))
            })?;

        let words: Vec<&str> = problem.split_whitespace().collect();

        for (i, word) in words.iter().enumerate() {
            let normalized =
                word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_lowercase();

            if normalized == "function" && i + 1 < words.len() {
                let candidate =
                    words[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                if !candidate.is_empty()
                    && candidate.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    return Some((path.to_string(), candidate.to_string()));
                }
            }
        }

        for (i, word) in words.iter().enumerate() {
            let normalized =
                word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_lowercase();

            if normalized == "patch" && i + 1 < words.len() {
                let candidate =
                    words[i + 1].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');

                if !candidate.is_empty()
                    && candidate.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && candidate != "the"
                    && candidate != "function"
                    && candidate != "so"
                    && candidate != "to"
                {
                    return Some((path.to_string(), candidate.to_string()));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(problem: &str) -> DirectiveInputs {
        DirectiveInputs {
            problem: problem.to_string(),
            problem_lower: problem.to_lowercase(),
            ..Default::default()
        }
    }

    #[test]
    fn search_code_relative_path_binds_exactly() {
        let i = input("SEARCH_CODE_OK:vendor/openfang:Cargo.toml");
        let out = DirectiveStrategyCompiler::compile(&i).unwrap();
        assert_eq!(out, vec!["search_code:vendor/openfang:Cargo.toml"]);
    }

    #[test]
    fn search_code_absolute_path_binds_exactly() {
        let i = input("SEARCH_CODE_OK:/root/AthenaFusionX/vendor/openfang:Cargo.toml");
        let out = DirectiveStrategyCompiler::compile(&i).unwrap();
        assert_eq!(out, vec!["search_code:/root/AthenaFusionX/vendor/openfang:Cargo.toml"]);
    }

    #[test]
    fn read_targets_preserve_order() {
        let mut i = input("READ_OK:a\nREAD_OK:b\nREAD_OK:c");
        i.read_targets = vec!["a".into(), "b".into(), "c".into()];

        let out = DirectiveStrategyCompiler::compile(&i).unwrap();

        assert_eq!(out, vec!["read_file:a", "read_file:b", "read_file:c"]);
    }

    #[test]
    fn patch_precedes_explicit_verification_command() {
        let i = input(
            "patch function foo in apps/math_utils/src/lib.rs; \
             execute `cargo test -p math_utils`",
        );

        let out = DirectiveStrategyCompiler::compile(&i).unwrap();

        assert_eq!(
            out,
            vec![
                "patch_function:apps/math_utils/src/lib.rs:foo",
                "read_file:apps/math_utils/src/lib.rs",
                "replace_lines:apps/math_utils/src/lib.rs",
            ]
        );
    }

    #[test]
    fn command_is_used_when_no_higher_priority_strategy_exists() {
        let mut i = input("execute `cargo check -p math_utils`");
        i.required_command = Some("cargo check -p math_utils".into());

        let out = DirectiveStrategyCompiler::compile(&i).unwrap();

        assert_eq!(out, vec!["execute_command:cargo check -p math_utils"]);
    }

    #[test]
    fn no_deterministic_directive_returns_none() {
        let i = input("Analyze the architecture and propose several approaches.");
        assert!(DirectiveStrategyCompiler::compile(&i).is_none());
    }
}
