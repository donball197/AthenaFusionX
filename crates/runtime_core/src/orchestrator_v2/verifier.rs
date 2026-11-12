use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerificationResult {
    Success,
    Retry(String),
    Failure(String),
    Escalate(String),
}

pub struct Verifier;

fn read_evidence_matches_path(evidence: &str, expected_path: &str) -> bool {
    evidence.lines().any(|line| {
        if let Some(path) = line.strip_prefix("[READ_OK]") {
            return crate::evidence_paths::paths_equivalent(path.trim(), expected_path);
        }

        if let Some(rest) = line.strip_prefix("[READ_CHUNK_OK] File: ") {
            return rest
                .split(" | ")
                .next()
                .map(|p| crate::evidence_paths::paths_equivalent(p, expected_path))
                .unwrap_or(false);
        }

        // A path-qualified read failure (e.g. ENOENT) or a path-scoped
        // search that found nothing is still a completed, evidenced
        // inspection of the target location -- the location was checked
        // and found empty/absent. This node-level DAG verifier is a
        // separate, independent copy of the same READ_OK matching logic
        // already extended in outcome_verifier.rs's ReadPathAcquired
        // check and cortical_loop.rs's confirmed-absence terminal check;
        // without this, the cortex can correctly complete a node on
        // confirmed-absence evidence, only for this sibling verifier to
        // immediately reject the same evidence as insufficient.
        if let Some(rest) = line.strip_prefix("[ACTION FAILED] path=") {
            // CONFIRMED-ABSENCE ONLY (see evidence_paths::is_confirmed_absence).
            return rest
                .split(" OS rejected action")
                .next()
                .map(|p| crate::evidence_paths::paths_equivalent(p, expected_path))
                .unwrap_or(false)
                && crate::evidence_paths::is_confirmed_absence(rest);
        }

        if let Some(rest) = line.strip_prefix("[SEARCH_CODE_OK] Path: ") {
            return rest
                .split(" | ")
                .next()
                .map(|p| crate::evidence_paths::paths_equivalent(p, expected_path))
                .unwrap_or(false);
        }

        false
    })
}

impl Verifier {
    /// LocalDiagnosis Layer: Cheap, deterministic heuristic matching.
    pub fn diagnose_output(stderr: &str, stdout: &str) -> VerificationResult {
        let combined = format!("{} {}", stderr, stdout).to_lowercase();

        // 1. Transient Faults -> Yields a localized Retry
        if combined.contains("timeout")
            || combined.contains("connection reset")
            || combined.contains("rate limit")
            || combined.contains("503 service unavailable")
            || combined.contains("429 too many requests")
        {
            return VerificationResult::Retry("Transient network/API fault detected.".into());
        }

        // 2. Deterministic Faults -> Immediate Escalation
        if combined.contains("error:")
            || combined.contains("cannot find crate")
            || combined.contains("borrowed value")
            || combined.contains("syntax error")
            || combined.contains("test failed")
            || combined.contains("panicked at")
        {
            return VerificationResult::Escalate(
                "Deterministic compilation/logic fault detected.".into(),
            );
        }

        // 3. Ambiguous Anomaly -> Escalate
        if !stderr.trim().is_empty() {
            return VerificationResult::Escalate("Unknown stderr anomaly detected.".into());
        }

        VerificationResult::Success
    }

    pub async fn evaluate(stdout: &str, stderr: &str, criteria: &[String]) -> VerificationResult {
        let diagnosis = Self::diagnose_output(stderr, stdout);
        if diagnosis != VerificationResult::Success {
            return diagnosis;
        }
        if criteria.is_empty() {
            return VerificationResult::Escalate("Node has no explicit success criteria.".into());
        }

        Self::evaluate_contract_criteria(stdout, criteria)
    }

    /// True when evidence holds a successful `[CMD_OK] Command:` record whose
    /// command contains the required command's first line (workspace-cd prefix
    /// stripped, whitespace-normalized), followed by `Status: exit status: 0`.
    pub fn command_ok_for(evidence: &str, command: &str) -> bool {
        let norm = |c: &str| {
            crate::cmd_match::strip_workspace_cd_prefix(c.trim())
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let want = norm(command.lines().next().unwrap_or(""));
        if want.is_empty() {
            return true;
        }
        let lines: Vec<&str> = evidence.lines().collect();
        lines.iter().enumerate().any(|(i, l)| {
            let Some(rest) = l.trim_start().strip_prefix("[CMD_OK] Command:") else {
                return false;
            };
            norm(rest).contains(&want)
                && lines[i + 1..]
                    .iter()
                    .take_while(|l| !crate::evidence_paths::is_marker_line(l))
                    .any(|l| l.trim() == "Status: exit status: 0")
        })
    }

    /// Deterministic contract verification based on explicit node criteria.
    ///
    /// Explicit success criteria are authoritative. Legacy node-id rules are
    /// retained only for older persisted DAG nodes that have no criteria.
    pub fn verify_contract(
        node_id: &str,
        evidence: &str,
        criteria: &[String],
    ) -> VerificationResult {
        if !criteria.is_empty() {
            return Self::evaluate_contract_criteria(evidence, criteria);
        }
        // Unknown nodes fail closed. A verifier must never silently
        // authorize an unrecognized node.
        VerificationResult::Escalate(format!(
            "No verification contract exists for node '{}'",
            node_id
        ))
    }

    fn evaluate_contract_criteria(evidence: &str, criteria: &[String]) -> VerificationResult {
        if criteria.is_empty() {
            return VerificationResult::Escalate(
                "Cannot verify node: success criteria are empty.".into(),
            );
        }

        for criterion in criteria {
            let criterion = criterion.trim();

            let satisfied = if criterion == "NODE_OBJECTIVE_EXECUTED_AND_VERIFIED" {
                evidence.lines().any(|l| l.trim_start().starts_with("[DAG NODE COMPLETE]"))
            } else if let Some(path) = criterion.strip_prefix("READ_OK:") {
                read_evidence_matches_path(evidence, path.trim())
            } else if let Some(target) = criterion.strip_prefix("READ_FUNCTION_OK:") {
                let mut parts = target.splitn(2, ':');
                let path = parts.next().unwrap_or("").trim();
                let func = parts.next().unwrap_or("").trim();
                crate::evidence_markers::read_function_marker_satisfies(evidence, path, func)
            } else if let Some(target) = criterion.strip_prefix("SEARCH_CODE_OK:") {
                crate::evidence_markers::search_term_satisfied(evidence, target)
            } else if let Some(target) = criterion.strip_prefix("SEARCH_CHECKED_OK:") {
                crate::evidence_markers::search_checked_satisfied(evidence, target)
            } else if criterion == "ANALYSIS_OK" {
                evidence.lines().any(|l| l.trim_start().starts_with("[HALT]"))
            } else if criterion == "SEARCH_MEMORY_OK" {
                evidence.lines().any(|l| l.trim_start().starts_with("[SEARCH_MEMORY_OK]"))
            } else if let Some(path) = criterion.strip_prefix("WRITE_OK:") {
                let target = path.trim();
                crate::evidence_markers::write_path_satisfied(evidence, target)
            } else if criterion == "CMD_OK" {
                // A bare [CMD_OK] substring is not proof of success. The
                // verifier must require the marker on its own line followed
                // by a zero exit status, matching the emission shape of both
                // the executor's ExecuteCommand branch:
                //     [CMD_OK] Command: <cmd>
                //     Status: exit status: 0
                //
                // Without this binding, literal text "[CMD_OK]" appearing in
                // a file that was read into evidence (via [READ_OK]) would
                // satisfy the criterion without any command having run.
                //
                // The original check required the Status line to be the
                // LITERAL NEXT line after [CMD_OK], which assumes a
                // single-line command. The executor's format string embeds
                // command.trim() verbatim before the Status line, so a
                // multi-line command (e.g. python3 -c with a multi-line
                // script) pushes Status several lines further down.
                // Confirmed root cause (2026-09-26): the ALL/ANY duality
                // mission's verification Python script executed successfully
                // (STDOUT confirmed all checks passed) but was rejected with
                // "Mandatory success criterion not satisfied: CMD_OK" because
                // the line immediately after [CMD_OK] was the script's own
                // first line of source, not "Status: exit status: 0".
                //
                // Fix: scan forward from [CMD_OK] for the Status line,
                // bounded by the next bracketed marker so a Status line
                // belonging to a later, unrelated command block can't be
                // matched instead.
                let lines: Vec<&str> = evidence.lines().collect();
                lines.iter().enumerate().any(|(i, line)| {
                    // Trimmed: an indented [CMD_OK] counts. Scan bounded by
                    // is_marker_line so bracketed command output does not
                    // truncate the scan early.
                    line.trim_start().starts_with("[CMD_OK]")
                        && lines[i + 1..]
                            .iter()
                            .take_while(|l| !crate::evidence_paths::is_marker_line(l))
                            .any(|l| l.trim() == "Status: exit status: 0")
                })
            } else if criterion == "PATCH_FUNCTION_OK" {
                evidence.lines().any(|l| {
                    let t = l.trim_start();
                    t.starts_with("[PATCH_FUNCTION_OK]") || t.starts_with("[PATCH_IDEMPOTENT_OK]")
                })
            } else if let Some(func) = criterion.strip_prefix("PATCH_FUNCTION_OK:") {
                let mut parts = func.splitn(2, ':');
                let path_or_name = parts.next().unwrap_or("").trim();
                let maybe_name = parts.next().unwrap_or("").trim();
                if path_or_name.is_empty() {
                    false
                } else if maybe_name.is_empty() {
                    // Single-field form. Bind the function name to a real
                    // [PATCH_FUNCTION_OK] or [PATCH_IDEMPOTENT_OK] record,
                    // so the name appearing in unrelated read text does
                    // not satisfy this criterion.
                    evidence.lines().any(|l| {
                        let t = l.trim_start();
                        let rest = match t.strip_prefix("[PATCH_FUNCTION_OK]") {
                            Some(r) => r,
                            None => match t.strip_prefix("[PATCH_IDEMPOTENT_OK]") {
                                Some(r) => r,
                                None => return false,
                            },
                        };
                        rest.contains(path_or_name)
                    })
                } else {
                    // Two-field form (<path>:<fn>). Delegate to the shared
                    // record-scoped matcher, which binds kind + path + fn.
                    crate::evidence_markers::patch_path_satisfied(
                        evidence,
                        "PATCH_FUNCTION_OK",
                        path_or_name,
                        Some(maybe_name),
                    )
                }
            } else if criterion == "DIFF_PATCH_OK" {
                evidence.lines().any(|l| l.trim_start().starts_with("[DIFF_PATCH_OK]"))
            } else if let Some(path) = criterion.strip_prefix("DIFF_PATCH_OK:") {
                crate::evidence_markers::patch_path_satisfied(evidence, "DIFF_PATCH_OK", path, None)
            } else if criterion == "REPLACE_LINES_OK" {
                evidence.lines().any(|l| l.trim_start().starts_with("[REPLACE_LINES_OK]"))
            } else if let Some(path) = criterion.strip_prefix("REPLACE_LINES_OK:") {
                crate::evidence_markers::patch_path_satisfied(
                    evidence,
                    "REPLACE_LINES_OK",
                    path,
                    None,
                )
            } else if criterion == "DAG_LINEAGE_OK" {
                crate::outcome_verifier::verify_lineage_provenance(evidence)
            } else {
                return VerificationResult::Escalate(format!(
                    "Unknown success criterion: {}",
                    criterion
                ));
            };

            if !satisfied {
                return VerificationResult::Retry(format!(
                    "Mandatory success criterion not satisfied: {}",
                    criterion
                ));
            }
        }

        VerificationResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::{VerificationResult, Verifier};

    #[test]
    fn search_memory_ok_requires_physical_evidence() {
        let criteria = vec!["SEARCH_MEMORY_OK".to_string()];

        let success = Verifier::verify_contract(
            "task_1",
            "[SEARCH_MEMORY_OK] Found semantic context: host observations involving uptime, filesystem, available memory, and load average",
            &criteria,
        );
        assert_eq!(success, VerificationResult::Success);

        let empty = Verifier::verify_contract(
            "task_1",
            "[SEARCH_MEMORY_EMPTY] Semantic search completed successfully but returned zero matches",
            &criteria,
        );
        assert!(matches!(empty, VerificationResult::Retry(_)));

        let unrelated =
            Verifier::verify_contract("task_1", "[SEARCH_CODE_OK] host observations", &criteria);
        assert!(matches!(unrelated, VerificationResult::Retry(_)));
    }

    #[test]
    fn checked_criterion_accepts_no_match_but_strict_does_not() {
        let no_match = concat!(
            "[SEARCH_CODE_OK] Path: bins | Query: 'todo!'\n",
            "No code matches found in specified path.\n"
        );

        let checked = vec!["SEARCH_CHECKED_OK:bins:todo!".to_string()];
        assert_eq!(
            Verifier::verify_contract("t", no_match, &checked),
            VerificationResult::Success,
            "checked-search must accept a documented no-match"
        );

        let strict = vec!["SEARCH_CODE_OK:bins:todo!".to_string()];
        assert!(
            matches!(
                Verifier::verify_contract("t", no_match, &strict),
                VerificationResult::Retry(_)
            ),
            "strict search must reject a documented no-match"
        );
    }

    #[test]
    fn checked_criterion_rejects_bare_marker() {
        let criteria = vec!["SEARCH_CHECKED_OK:bins:todo!".to_string()];
        let bare = "[SEARCH_CODE_OK] Path: bins | Query: 'todo!'\n";
        assert!(matches!(
            Verifier::verify_contract("t", bare, &criteria),
            VerificationResult::Retry(_)
        ));
    }

    #[test]
    fn bins_criterion_rejects_crates_only_search() {
        let criteria = vec!["SEARCH_CODE_OK:bins:unimplemented!".to_string()];
        let evidence = "[SEARCH_CODE_OK] Path: crates | Query: 'unimplemented!'\nMATCHES:\nfoo.rs:1: unimplemented!()\n";
        let result = Verifier::verify_contract("t", evidence, &criteria);
        assert!(
            matches!(result, VerificationResult::Retry(_)),
            "a crates-only search must not satisfy a bins-bound criterion; got {:?}",
            result
        );
    }

    #[test]
    fn bins_criterion_accepts_bins_search() {
        let criteria = vec!["SEARCH_CODE_OK:bins:unimplemented!".to_string()];
        let evidence = "[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\nMATCHES:\nmain.rs:1: unimplemented!()\n";
        assert_eq!(
            Verifier::verify_contract("t", evidence, &criteria),
            VerificationResult::Success
        );
    }

    #[test]
    fn crates_and_bins_criteria_require_both_searches() {
        let criteria = vec![
            "SEARCH_CODE_OK:crates:unimplemented!".to_string(),
            "SEARCH_CODE_OK:bins:unimplemented!".to_string(),
        ];
        let only_crates = "[SEARCH_CODE_OK] Path: crates | Query: 'unimplemented!'\nMATCHES:\nlib.rs:1: unimplemented!()\n";
        assert!(
            matches!(
                Verifier::verify_contract("t", only_crates, &criteria),
                VerificationResult::Retry(_)
            ),
            "one crates search must not satisfy both crates and bins criteria"
        );

        let both = concat!(
            "[SEARCH_CODE_OK] Path: crates | Query: 'unimplemented!'\nMATCHES:\nlib.rs:1: unimplemented!()\n",
            "[SEARCH_CODE_OK] Path: bins | Query: 'unimplemented!'\nMATCHES:\nmain.rs:1: unimplemented!()\n",
        );
        assert_eq!(Verifier::verify_contract("t", both, &criteria), VerificationResult::Success);
    }

    #[test]
    fn idempotent_patch_satisfies_node_verifier() {
        let criteria = vec!["PATCH_FUNCTION_OK:calculate_total".to_string()];
        let evidence = "[PATCH_IDEMPOTENT_OK] src/math.rs:calculate_total\nPatchFunction already satisfied; source unchanged.";

        let result = Verifier::verify_contract("task_mutate", evidence, &criteria);
        assert_eq!(result, VerificationResult::Success);
    }

    #[test]
    fn read_function_rejects_marker_and_target_from_different_records() {
        let criteria = vec!["READ_FUNCTION_OK:crates/runtime_core/src/bar.rs:target_b".to_string()];

        let evidence = concat!(
            "[READ_FUNCTION_OK] crates/runtime_core/src/foo.rs:actual_a\n",
            "File: crates/runtime_core/src/foo.rs | Function: actual_a\n",
            "EXACT AST SOURCE:\n",
            "fn actual_a() {}\n",
            "Observed reference to crates/runtime_core/src/bar.rs\n",
            "Discussion mentions target_b\n",
        );

        let result = Verifier::verify_contract("read_function_contract_test", evidence, &criteria);

        assert!(matches!(result, VerificationResult::Retry(_)));
    }

    #[test]
    fn read_function_accepts_exact_marker_record() {
        let criteria = vec!["READ_FUNCTION_OK:crates/runtime_core/src/bar.rs:target_b".to_string()];

        let evidence = concat!(
            "[READ_FUNCTION_OK] crates/runtime_core/src/bar.rs:target_b\n",
            "File: crates/runtime_core/src/bar.rs | Function: target_b\n",
            "EXACT AST SOURCE:\n",
            "fn target_b() {}\n",
        );

        let result = Verifier::verify_contract("read_function_contract_test", evidence, &criteria);

        assert_eq!(result, VerificationResult::Success);
    }

    #[test]
    fn patch_function_ok_verifies_successfully() {
        let criteria = vec!["PATCH_FUNCTION_OK:calculate_total".to_string()];
        let evidence = "[PATCH_FUNCTION_OK] Updated function 'calculate_total' in src/math.rs";

        let result = Verifier::verify_contract("task_mutate", evidence, &criteria);
        assert_eq!(result, VerificationResult::Success);

        let wrong_func_criteria = vec!["PATCH_FUNCTION_OK:calculate_discount".to_string()];
        let failed = Verifier::verify_contract("task_mutate", evidence, &wrong_func_criteria);
        assert!(matches!(failed, VerificationResult::Retry(_)));
    }

    #[test]
    fn diff_patch_ok_verifies_successfully() {
        let criteria = vec!["DIFF_PATCH_OK:src/lib.rs".to_string()];
        let evidence = "[DIFF_PATCH_OK] Applied changes to src/lib.rs";

        let result = Verifier::verify_contract("task_mutate", evidence, &criteria);
        assert_eq!(result, VerificationResult::Success);

        let wrong_path_criteria = vec!["DIFF_PATCH_OK:src/main.rs".to_string()];
        let failed = Verifier::verify_contract("task_mutate", evidence, &wrong_path_criteria);
        assert!(matches!(failed, VerificationResult::Retry(_)));
    }

    #[test]
    fn dag_lineage_ok_verifies_with_content_hash() {
        use sha2::{Digest, Sha256};

        let criteria = vec!["DAG_LINEAGE_OK".to_string()];
        let payload = "[PATCH_FUNCTION_OK] Updated function 'route' in router.rs";
        let digest = format!("{:x}", Sha256::digest(payload.as_bytes()));
        let evidence = format!(
            "[DAG LINEAGE INHERITED EVIDENCE sha256:{}]\n{}\n[DAG LINEAGE COMPLETE sha256:{}]",
            digest, payload, digest
        );

        let result = Verifier::verify_contract("task_verify", &evidence, &criteria);
        assert_eq!(result, VerificationResult::Success);

        let tampered = format!(
            "[DAG LINEAGE INHERITED EVIDENCE sha256:{}]\n{} modified\n[DAG LINEAGE COMPLETE sha256:{}]",
            digest, payload, digest
        );
        let failed = Verifier::verify_contract("task_verify", &tampered, &criteria);
        assert!(matches!(failed, VerificationResult::Retry(_)));
    }
}

#[cfg(test)]
mod read_chunk_verifier_tests {
    use super::{VerificationResult, Verifier};

    fn criterion() -> Vec<String> {
        vec!["READ_OK:crates/runtime_core/src/cortical_loop.rs".to_string()]
    }

    #[test]
    fn read_ok_accepts_exact_whole_file_marker() {
        let evidence = "[READ_OK] crates/runtime_core/src/cortical_loop.rs\n";

        assert!(matches!(
            Verifier::verify_contract("read_chunk_contract_test", evidence, &criterion(),),
            VerificationResult::Success
        ));
    }

    #[test]
    fn read_ok_accepts_exact_chunk_marker() {
        let evidence =
            "[READ_CHUNK_OK] File: crates/runtime_core/src/cortical_loop.rs | Lines 1-100 of 500\n";

        assert!(matches!(
            Verifier::verify_contract("read_chunk_contract_test", evidence, &criterion(),),
            VerificationResult::Success
        ));
    }

    #[test]
    fn read_ok_rejects_chunk_for_different_file() {
        let evidence =
            "[READ_CHUNK_OK] File: crates/runtime_core/src/other.rs | Lines 1-100 of 500\n";

        assert!(!matches!(
            Verifier::verify_contract("read_chunk_contract_test", evidence, &criterion(),),
            VerificationResult::Success
        ));
    }

    #[test]
    fn read_ok_rejects_whole_file_marker_for_different_file() {
        let evidence = "[READ_OK] crates/runtime_core/src/other.rs\n";

        assert!(!matches!(
            Verifier::verify_contract("read_chunk_contract_test", evidence, &criterion(),),
            VerificationResult::Success
        ));
    }

    #[test]
    fn read_ok_rejects_marker_and_path_from_different_records() {
        let evidence = concat!(
            "[READ_CHUNK_OK] File: crates/runtime_core/src/other.rs | Lines 1-100 of 500\n",
            "Observed reference to crates/runtime_core/src/cortical_loop.rs\n",
        );

        assert!(!matches!(
            Verifier::verify_contract("read_chunk_contract_test", evidence, &criterion(),),
            VerificationResult::Success
        ));
    }
}
