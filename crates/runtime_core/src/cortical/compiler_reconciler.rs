// crates/runtime_core/src/cortical/compiler_reconciler.rs

use crate::cognition::epistemic_session::EpistemicSession;
use crate::cognition_graph::{EdgeRelation, NodeKind};
use crate::compile_remediation::{
    format_compile_failure_evidence, CompileDecision, COMPILE_RETRY_BUDGET,
};
use crate::cortex::compiler_gate::{CompilerGate, CompilerGateResult, RustVerificationTarget};
use crate::cortical_loop::CorticalLoop;
use anyhow::Result;

pub(crate) enum CompilerVerificationOutcome {
    Passed,
    NeedsStrategize,
}

impl CorticalLoop {
    /// Resolves mutated package targets, invokes compiler verification,
    /// and manages retry / revert remediation state machines.
    pub(crate) async fn verify_and_reconcile_compiler(
        &mut self,
        session: &mut EpistemicSession,
        mutation_paths: &[String],
        evidence_checkpoint: usize,
        latest_evidence: &mut String,
    ) -> Result<CompilerVerificationOutcome> {
        self.log(
            "\x1b[36m>> [COMPILER CRITIC] Resolving mutated Rust package(s) via cargo metadata...\x1b[0m"
                .to_string(),
        );

        let target = match CompilerGate::resolve_target(mutation_paths).await {
            Ok(target) => target,
            Err(error) => {
                let message = format!("[COMPILER_FAULT] {}", error);
                self.log(format!("\x1b[1;31m>> [COMPILER CRITIC] {}\x1b[0m", message));
                session.local_evidence.truncate(evidence_checkpoint);
                session.local_evidence.push_str(&message);
                session.local_evidence.push('\n');
                return Ok(CompilerVerificationOutcome::NeedsStrategize);
            }
        };

        let packages = match &target {
            RustVerificationTarget::WorkspacePackages { packages, .. } => packages.clone(),
            RustVerificationTarget::StandaloneFiles { files } => {
                files.iter().map(|f| f.display().to_string()).collect()
            }
            RustVerificationTarget::NoVerificationNeeded => Vec::new(),
        };

        self.log(format!(
            "\x1b[36m>> [COMPILER CRITIC] Verification target(s): {}\x1b[0m",
            if packages.is_empty() { "none".to_string() } else { packages.join(", ") }
        ));

        match &target {
            RustVerificationTarget::WorkspacePackages { workspace_manifest, .. } => {
                self.log(format!(
                    "\x1b[36m>> [COMPILER CRITIC] Running package-scoped cargo check through CompilerGate using {}...\x1b[0m",
                    workspace_manifest.display()
                ));
            }
            RustVerificationTarget::StandaloneFiles { files } => {
                let names: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
                self.log(format!(
                    "\x1b[36m>> [COMPILER CRITIC] Running standalone rustc verification through CompilerGate for [{}]...\x1b[0m",
                    names.join(", ")
                ));
            }
            RustVerificationTarget::NoVerificationNeeded => {
                self.log(
                    "\x1b[36m>> [COMPILER CRITIC] No physical compiler verification required.\x1b[0m"
                        .to_string(),
                );
            }
        }

        let compile_result = CompilerGate::verify_target(&target).await;

        match compile_result {
            CompilerGateResult::Success { success_marker, targets_checked: _ } => {
                session.compile_remediation.reset();

                let mut success_evidence = String::new();
                success_evidence.push('\n');
                success_evidence.push_str(&success_marker);

                latest_evidence.push_str(&success_evidence);
                session.local_evidence.push_str(&success_evidence);

                self.log(
                    "\x1b[32m>> [COMPILER CRITIC] Physical compiler verification OK. Proceeding to LLM Evaluator.\x1b[0m"
                        .to_string(),
                );
                Ok(CompilerVerificationOutcome::Passed)
            }

            CompilerGateResult::CompilationFailed {
                error_preview: err_preview,
                failed_targets: _,
            } => {
                let decision = session.compile_remediation.classify(false);

                match &decision {
                    CompileDecision::RetainForRetry { iteration } => {
                        session.active_mutation_paths = mutation_paths.to_vec();
                        self.log(format!(
                            "\x1b[1;33m>> [COMPILER CRITIC] Physical compile verification FAILED (iteration {}/{}). Retaining mutation; feeding diagnostic to strategist.\x1b[0m",
                            iteration,
                            COMPILE_RETRY_BUDGET
                        ));

                        if let Some(evidence) =
                            format_compile_failure_evidence(&decision, &packages, &err_preview)
                        {
                            session.local_evidence.push_str(&evidence);
                        }

                        Ok(CompilerVerificationOutcome::NeedsStrategize)
                    }

                    CompileDecision::BudgetExhausted { total_iterations } => {
                        self.log(format!(
                            "\x1b[1;31m>> [COMPILER CRITIC] Compile-fail retry budget exhausted after {} iterations. Reverting mutations.\x1b[0m",
                            total_iterations
                        ));

                        let mut revert_all_succeeded = true;
                        for path in mutation_paths {
                            if crate::podman::PodmanShield::execute_argv(
                                "git",
                                &["checkout", "--", path],
                            )
                            .is_err()
                            {
                                revert_all_succeeded = false;
                            }
                        }

                        session.local_evidence.truncate(evidence_checkpoint);

                        if let Some(evidence) =
                            format_compile_failure_evidence(&decision, &packages, &err_preview)
                        {
                            session.local_evidence.push_str(&evidence);
                        }
                        session.local_evidence.push('\n');

                        if revert_all_succeeded {
                            session.compile_remediation.reset();
                        } else {
                            session.local_evidence.push_str(
                                "[COMPILE_REVERT_FAILED] One or more `git checkout` reverts did not succeed. Source may still be broken. The remediation episode remains open.\n",
                            );
                        }

                        if let Some(h_id) = session.best_hypothesis_id {
                            let obs_id = session.graph.add_node(
                                NodeKind::Observation,
                                if revert_all_succeeded {
                                    "[COMPILE_FAIL] Patch failed physical compiler verification. Reverted."
                                } else {
                                    "[COMPILE_REVERT_FAILED] Patch failed; revert did not complete."
                                },
                                1.0,
                                90,
                            );

                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);

                            session.graph.propagate_beliefs();
                        }

                        Ok(CompilerVerificationOutcome::NeedsStrategize)
                    }

                    CompileDecision::Success => {
                        self.log(
                            "\x1b[1;31m>> [COMPILER CRITIC] Internal inconsistency: Success decision on failed compile. Treating as exhaustion.\x1b[0m"
                                .to_string(),
                        );

                        let mut revert_all_succeeded = true;
                        for path in mutation_paths {
                            if crate::podman::PodmanShield::execute_argv(
                                "git",
                                &["checkout", "--", path],
                            )
                            .is_err()
                            {
                                revert_all_succeeded = false;
                            }
                        }

                        if revert_all_succeeded {
                            session.compile_remediation.reset();
                        } else {
                            session.local_evidence.push_str(
                                "[COMPILE_REVERT_FAILED] Internal inconsistency path: revert did not complete. Episode remains open.\n",
                            );
                        }

                        Ok(CompilerVerificationOutcome::NeedsStrategize)
                    }
                }
            }

            CompilerGateResult::ExecutionFault { error_message } => {
                let message = format!("[COMPILER_FAULT] {}", error_message);

                self.log(format!("\x1b[1;31m>> [COMPILER CRITIC] {}\x1b[0m", message));

                session.local_evidence.truncate(evidence_checkpoint);

                let decision = session.compile_remediation.classify(false);

                match &decision {
                    CompileDecision::RetainForRetry { iteration } => {
                        session.active_mutation_paths = mutation_paths.to_vec();

                        session.local_evidence.push_str(&format!(
                            "[COMPILER_FAULT_RETRY] iteration {}/{}\n",
                            iteration, COMPILE_RETRY_BUDGET
                        ));
                        session.local_evidence.push_str(&message);
                        session.local_evidence.push('\n');

                        Ok(CompilerVerificationOutcome::NeedsStrategize)
                    }

                    CompileDecision::BudgetExhausted { total_iterations } => {
                        session.local_evidence.push_str(&format!(
                            "[COMPILER_FAULT_BUDGET_EXHAUSTED] {} failed compiler-verification attempts.\n",
                            total_iterations
                        ));
                        session.local_evidence.push_str(&message);
                        session.local_evidence.push('\n');

                        let mut revert_all_succeeded = true;

                        for path in mutation_paths {
                            if crate::podman::PodmanShield::execute_argv(
                                "git",
                                &["checkout", "--", path],
                            )
                            .is_err()
                            {
                                revert_all_succeeded = false;
                            }
                        }

                        if revert_all_succeeded {
                            session.compile_remediation.reset();
                            session.active_mutation_paths.clear();
                        } else {
                            session.local_evidence.push_str(
                                "[COMPILE_REVERT_FAILED] One or more `git checkout` reverts did not succeed. Source may still be broken. The remediation episode remains open.\n",
                            );
                        }

                        if let Some(h_id) = session.best_hypothesis_id {
                            let obs_id = session.graph.add_node(
                                NodeKind::Observation,
                                if revert_all_succeeded {
                                    "[COMPILER_FAULT] Compiler verification could not launch; mutation reverted."
                                } else {
                                    "[COMPILER_FAULT] Compiler verification could not launch; revert failed."
                                },
                                1.0,
                                90,
                            );

                            session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.99);

                            session.graph.propagate_beliefs();
                        }

                        Ok(CompilerVerificationOutcome::NeedsStrategize)
                    }

                    CompileDecision::Success => {
                        session.local_evidence.push_str(&message);
                        session.local_evidence.push('\n');
                        Ok(CompilerVerificationOutcome::NeedsStrategize)
                    }
                }
            }
        }
    }
}
