use crate::agent::RefactorAgent;
use crate::evolution::{
    compare_security_findings, evaluate_v1, source_fingerprint, stable_fingerprint, study_snapshot,
    BuildIdentity, CandidateIdentity, CapabilityEvidence, Comparison, DryRunReport,
    EvolutionRecord, FunctionalEvidence, HumanApproval, Hypothesis, OptimizationCandidate,
    OptimizationReport,
};
use crate::polyglot::PolyglotScanner;
use anyhow::{anyhow, Context};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Target-aware functional validation profile.
///
/// The profile is deliberately small in V1. Security validation remains
/// owned by the existing audit pipeline and will be attached later as a
/// separate hard gate.
#[derive(Debug, Clone)]
pub struct BuildProfile {
    pub workspace_root: PathBuf,
    pub manifest_path: PathBuf,
}

impl BuildProfile {
    /// Discover the nearest Cargo manifest for a source target.
    ///
    /// This avoids hard-coding apps/nexus_ast_tool/Cargo.toml into the
    /// optimization engine.
    pub fn discover<P: AsRef<Path>>(target: P) -> anyhow::Result<Self> {
        let target = target.as_ref();

        let start = if target.is_dir() {
            target
        } else {
            target.parent().ok_or_else(|| anyhow!("Target has no parent directory"))?
        };

        let manifest_path = find_nearest_manifest(start)
            .ok_or_else(|| anyhow!("No Cargo.toml found for target {}", target.display()))?;

        // CRITICAL: resolve the real workspace root, not just the member package.
        // Member crates that use `workspace = true` cannot be built in isolation.
        let workspace_root = find_workspace_root(&manifest_path);

        Ok(Self { workspace_root, manifest_path })
    }

    /// Run the target-aware build validation.
    pub fn check(&self) -> anyhow::Result<BuildValidation> {
        let output = Command::new("cargo")
            .args([
                "check",
                "--manifest-path",
                self.manifest_path
                    .to_str()
                    .ok_or_else(|| anyhow!("Manifest path is not valid UTF-8"))?,
            ])
            .current_dir(&self.workspace_root)
            .output()
            .with_context(|| {
                format!("Failed to execute cargo check for {}", self.manifest_path.display())
            })?;

        Ok(BuildValidation {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Run the target-aware test suite.
    pub fn test(&self) -> anyhow::Result<BuildValidation> {
        let output = Command::new("cargo")
            .args([
                "test",
                "--manifest-path",
                self.manifest_path
                    .to_str()
                    .ok_or_else(|| anyhow!("Manifest path is not valid UTF-8"))?,
            ])
            .current_dir(&self.workspace_root)
            .output()
            .with_context(|| {
                format!("Failed to execute cargo test for {}", self.manifest_path.display())
            })?;

        Ok(BuildValidation {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct BuildValidation {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl BuildValidation {
    /// Stable identity for functional-validation provenance.
    ///
    /// This deliberately excludes raw Cargo output because compiler/test
    /// output can contain nondeterministic paths, timing, and environment
    /// details. The identity is provenance metadata, not a cryptographic
    /// security primitive.
    pub fn identity(&self, manifest_path: &Path, command: &str) -> String {
        format!(
            "validation:v1|command={}|manifest={}|success={}|exit_code={:?}",
            command,
            manifest_path.display(),
            self.success,
            self.exit_code
        )
    }
}

fn find_nearest_manifest(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        let candidate = current.join("Cargo.toml");

        if candidate.is_file() {
            return Some(candidate);
        }

        if !current.pop() {
            return None;
        }
    }
}

/// Walk upward from a package manifest until we locate the real Cargo
/// workspace root (the directory whose Cargo.toml contains a `[workspace]`
/// table). Falls back to the package directory when no workspace is found.
///
/// This is required for isolation: member crates that inherit dependencies
/// via `workspace = true` cannot be built without the workspace root.
fn find_workspace_root(manifest_path: &Path) -> PathBuf {
    let package_dir =
        manifest_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));

    let mut current = package_dir.clone();

    loop {
        let candidate = current.join("Cargo.toml");
        if candidate.is_file() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                // Sufficient for the V1 isolation contract.
                // A full TOML parse would also work but adds a dependency
                // and is unnecessary for this gate.
                if content.contains("[workspace]") {
                    return current;
                }
            }
        }
        if !current.pop() {
            // No workspace found — treat the original package dir as root.
            return package_dir;
        }
    }
}

pub struct Supervisor;

impl Supervisor {
    /// Gate 11.6/11.7 report-only candidate generation.
    ///
    /// This phase:
    /// - reads the authoritative baseline
    /// - generates candidate payloads
    /// - persists a deterministic optimization report
    /// - NEVER mutates the authoritative checkout
    /// - NEVER performs a dry run
    /// - NEVER promotes a candidate
    pub fn self_optimize<P: AsRef<Path>, O: AsRef<Path>>(
        target_file: P,
        output: O,
    ) -> anyhow::Result<()> {
        let path = target_file.as_ref().canonicalize()?;
        let output = output.as_ref();

        println!("==> [Supervisor] REPORT-ONLY Candidate Analysis: {}", path.display());

        let profile = BuildProfile::discover(&path)?;

        println!("    [BuildProfile] Workspace: {}", profile.workspace_root.display());
        println!("    [BuildProfile] Manifest: {}", profile.manifest_path.display());

        let baseline_code = fs::read_to_string(&path)?;
        let baseline_fingerprint = source_fingerprint(&baseline_code);

        let baseline_report = crate::study::run_study_report(&profile.workspace_root)?;
        let baseline_snapshot = study_snapshot(&baseline_report);

        println!(
            "    [Baseline] Structural snapshot: {} files, {} functions, {} entities, {} dependencies, {:.2}% parse health",
            baseline_snapshot.source_files,
            baseline_snapshot.function_count,
            baseline_snapshot.entity_count,
            baseline_snapshot.dependency_count,
            baseline_snapshot.parse_health_percent
        );

        let rules_dir = profile.workspace_root.join("rules");
        let rules = PolyglotScanner::load_rules(&rules_dir);
        let baseline_security_findings =
            PolyglotScanner::scan_security_vulnerabilities(&profile.workspace_root, &rules);

        println!("    [Baseline] Security findings: {}", baseline_security_findings.len());

        let symbols = PolyglotScanner::scan_path(&path);
        let mut processed_symbols = HashSet::new();
        let mut candidates = Vec::new();

        for sym in symbols {
            if sym.language != crate::polyglot::SupportedLanguage::Rust {
                continue;
            }

            if !processed_symbols.insert(sym.name.clone()) {
                continue;
            }

            let snippet = match RefactorAgent::read_function_source(&path, &sym.name) {
                Ok(source) => source,
                Err(e) => {
                    println!(
                        "    [⚠️] Skipping '{}' — complete AST source extraction failed: {}",
                        sym.name, e
                    );
                    continue;
                }
            };

            println!(
                "\n    [?] Generating read-only candidate for '{}' (complete AST source)",
                sym.name
            );

            let optimized_body =
                match crate::client::ApiClient::generate_optimization(&sym.name, &snippet) {
                    Ok(body) => {
                        println!("        [🧠] Candidate optimization generated.");
                        body
                    }
                    Err(e) => {
                        println!(
                            "        [⚠️] Candidate generation failed for '{}': {}",
                            sym.name, e
                        );
                        continue;
                    }
                };

            // Deadly-accurate dependency gate: reject payloads that invent crates
            // not present in the original source. cargo check remains authoritative.
            if let Some(reason) =
                crate::mutation::payload_introduces_unknown_crate(&snippet, &optimized_body)
            {
                println!("        [❌] Candidate rejected pre-record: {}", reason);
                continue;
            }

            let candidate_index = candidates.len();

            let hypothesis = Hypothesis {
                target_symbol: sym.name.clone(),
                strategy: "ReplaceFunctionBody".into(),
                expected_capability_improvement:
                    "Candidate optimization generated by the optimization model; measurable improvement must be established by isolated dry-run evidence."
                        .into(),
                expected_structural_effect:
                    "Preserve repository structure while improving target behavior."
                        .into(),
                expected_security_effect:
                    "No increase in scanner-reported security findings."
                        .into(),
                proposed_payload: optimized_body.clone(),
            };

            let candidate_identity = CandidateIdentity {
                candidate_id: format!("candidate:v1|symbol={}|index={}", sym.name, candidate_index),
                baseline_id: format!(
                    "baseline:v1|manifest={}|source={}",
                    profile.manifest_path.display(),
                    baseline_fingerprint
                ),
            };

            let functional =
                FunctionalEvidence { build_passed: false, tests_passed: None, test_identity: None };

            let security =
                compare_security_findings(&baseline_security_findings, &baseline_security_findings);

            let comparison = Comparison {
                structural: crate::evolution::StructuralDelta::default(),
                capability: CapabilityEvidence {
                    measurable_improvement: false,
                    description:
                        "Report-only proposal. Capability improvement requires isolated dry-run evidence."
                            .into(),
                    evidence_identity: None,
                    source: None,
                },
            };

            let decision = evaluate_v1(&functional, &security, &comparison);

            let record = EvolutionRecord::new(
                BuildIdentity {
                    target: path.display().to_string(),
                    source_fingerprint: baseline_fingerprint.clone(),
                    session_id: "supervisor-report-v1".into(),
                },
                hypothesis,
                candidate_identity.clone(),
                functional,
                security,
                comparison,
                decision,
            );

            candidates.push(OptimizationCandidate {
                candidate_id: candidate_identity.candidate_id.clone(),
                target: path.display().to_string(),
                target_symbol: sym.name.clone(),
                strategy: "ReplaceFunctionBody".into(),
                proposed_payload: optimized_body,
                evolution: record,
            });

            println!("        [🧾] Proposal recorded: {}", candidate_identity.candidate_id);
        }

        let baseline_after = fs::read_to_string(&path)?;

        if baseline_after != baseline_code {
            return Err(anyhow!(
                "REPORT-ONLY INVARIANT VIOLATION: target source changed during candidate generation"
            ));
        }

        let report = OptimizationReport::new(
            BuildIdentity {
                target: path.display().to_string(),
                source_fingerprint: baseline_fingerprint,
                session_id: "supervisor-report-v1".into(),
            },
            baseline_security_findings.len(),
            candidates,
        );

        fs::create_dir_all(output)?;

        let report_path = output.join("optimization_report.json");
        let serialized = serde_json::to_string_pretty(&report)?;
        fs::write(&report_path, serialized)?;

        println!("\n==> [Supervisor] Report-only candidate analysis complete.");
        println!("    Generated candidates: {}", report.candidates.len());
        println!("    Report: {}", report_path.display());
        println!("    Report identity: {}", report.report_identity);
        println!("    Target mutated: NO");
        println!("    Dry run performed: NO");
        println!("    Promotion performed: NO");
        println!("    Human approval required before dry run: YES");

        Ok(())
    }

    /// Gate 11.8 isolated dry run.
    ///
    /// Preconditions:
    /// - persisted optimization report
    /// - valid Human Approval #1
    /// - approval bound to this exact report and candidate
    ///
    /// The authoritative checkout is never mutated.
    /// No promotion is performed.
    pub fn isolated_dry_run<R: AsRef<Path>, A: AsRef<Path>, O: AsRef<Path>>(
        report_path: R,
        approval_path: A,
        output: O,
    ) -> anyhow::Result<()> {
        let report_path = report_path.as_ref().canonicalize()?;
        let approval_path = approval_path.as_ref().canonicalize()?;
        let output = output.as_ref();

        println!("==> [Gate 11.8] ISOLATED DRY RUN");

        let report: OptimizationReport = serde_json::from_str(&fs::read_to_string(&report_path)?)?;

        if report.schema_version != crate::evolution::EVOLUTION_SCHEMA_VERSION {
            return Err(anyhow!(
                "Unsupported optimization report schema: {}",
                report.schema_version
            ));
        }

        if report.recompute_identity() != report.report_identity {
            return Err(anyhow!(
                "Optimization report identity mismatch; report may have been modified."
            ));
        }

        let approval: HumanApproval = serde_json::from_str(&fs::read_to_string(&approval_path)?)?;

        let candidate_id = &approval.candidate_id;

        if !approval.authorizes(candidate_id, &approval.report_identity) {
            return Err(anyhow!(
                "Human Approval #1 is invalid or does not bind to its declared candidate/report."
            ));
        }

        if approval.report_identity != report.report_identity {
            return Err(anyhow!("Human Approval #1 does not authorize this optimization report."));
        }

        let candidate = report.find_candidate(candidate_id).ok_or_else(|| {
            anyhow!(
                "Approved candidate '{}' does not exist in the optimization report.",
                candidate_id
            )
        })?;

        let authoritative_target = PathBuf::from(&candidate.target).canonicalize()?;

        let authoritative_source = fs::read_to_string(&authoritative_target)?;

        let authoritative_fingerprint = source_fingerprint(&authoritative_source);

        if authoritative_fingerprint != report.baseline.source_fingerprint {
            return Err(anyhow!(
                "AUTHORITATIVE SOURCE DRIFT: source fingerprint no longer matches report baseline."
            ));
        }

        if candidate.evolution.baseline.source_fingerprint != authoritative_fingerprint {
            return Err(anyhow!(
                "CANDIDATE BASELINE MISMATCH: candidate was not generated from the current authoritative source."
            ));
        }

        // Output must not be inside the authoritative workspace.
        let authoritative_workspace =
            BuildProfile::discover(&authoritative_target)?.workspace_root.canonicalize()?;

        let output_abs = if output.is_absolute() {
            output.to_path_buf()
        } else {
            std::env::current_dir()?.join(output)
        };

        let output_abs = output_abs.canonicalize().unwrap_or(output_abs);

        if output_abs.starts_with(&authoritative_workspace) {
            return Err(anyhow!(
                "Isolation violation: dry-run output cannot be inside the authoritative workspace."
            ));
        }

        fs::create_dir_all(&output_abs)?;

        let isolated_workspace =
            output_abs.join(format!("workspace_{}", stable_fingerprint(candidate_id.as_bytes())));

        if isolated_workspace.exists() {
            fs::remove_dir_all(&isolated_workspace)?;
        }

        copy_tree(&authoritative_workspace, &isolated_workspace)?;

        let relative_target = authoritative_target
            .strip_prefix(&authoritative_workspace)
            .map_err(|_| anyhow!("Target is not inside discovered authoritative workspace."))?;

        let isolated_target = isolated_workspace.join(relative_target);

        println!("    [Isolation] Authoritative workspace: {}", authoritative_workspace.display());
        println!("    [Isolation] Candidate workspace: {}", isolated_workspace.display());
        println!("    [Isolation] Target: {}", isolated_target.display());

        let agent = crate::agent::RefactorAgent::new();

        agent.apply_refactoring(
            &isolated_target,
            &candidate.target_symbol,
            &candidate.strategy,
            &candidate.proposed_payload,
        )?;

        let isolated_profile = BuildProfile::discover(&isolated_target)?;

        let build = isolated_profile.check()?;

        println!("    [Build] cargo check: {}", if build.success { "PASS" } else { "FAIL" });

        let build_identity = build.identity(&isolated_profile.manifest_path, "cargo check");

        let tests = if build.success { Some(isolated_profile.test()?) } else { None };

        let tests_passed = tests.as_ref().map(|v| v.success);

        let test_identity =
            tests.as_ref().map(|v| v.identity(&isolated_profile.manifest_path, "cargo test"));

        if let Some(test) = &tests {
            println!("    [Tests] cargo test: {}", if test.success { "PASS" } else { "FAIL" });
        } else {
            println!("    [Tests] Not executed because build failed.");
        }

        let isolated_rules_dir = isolated_workspace.join("rules");
        let isolated_rules = PolyglotScanner::load_rules(&isolated_rules_dir);

        let candidate_security_findings =
            PolyglotScanner::scan_security_vulnerabilities(&isolated_workspace, &isolated_rules);

        // Re-scan the authoritative workspace read-only at dry-run time.
        let authoritative_rules_dir = authoritative_workspace.join("rules");

        let authoritative_rules = PolyglotScanner::load_rules(&authoritative_rules_dir);

        let authoritative_security_findings = PolyglotScanner::scan_security_vulnerabilities(
            &authoritative_workspace,
            &authoritative_rules,
        );

        let security = compare_security_findings(
            &authoritative_security_findings,
            &candidate_security_findings,
        );

        if matches!(security.status, crate::evolution::SecurityStatus::Fail) {
            println!("    [Security] FAIL — regression detected.");
        } else if matches!(security.status, crate::evolution::SecurityStatus::Unknown) {
            println!("    [Security] UNKNOWN — insufficient evidence.");
        } else {
            println!("    [Security] PASS");
        }

        let final_authoritative_source = fs::read_to_string(&authoritative_target)?;

        let authoritative_source_unchanged =
            source_fingerprint(&final_authoritative_source) == report.baseline.source_fingerprint;

        if !authoritative_source_unchanged {
            return Err(anyhow!("AUTHORITATIVE CHECKOUT CHANGED DURING DRY RUN."));
        }

        // The isolated candidate MUST differ from the authoritative target
        // when the mutation actually changed source. If it does not, that is
        // still useful evidence, but it is not promotion.
        let isolated_source = fs::read_to_string(&isolated_target)?;

        let target_mutated = isolated_source != authoritative_source;

        let dry_run = DryRunReport::new(
            report.report_identity.clone(),
            approval.approval_id.clone(),
            candidate_id.clone(),
            isolated_workspace.display().to_string(),
            build.success,
            build_identity,
            tests_passed,
            test_identity,
            authoritative_security_findings.len(),
            candidate_security_findings.len(),
            security.regression,
            authoritative_source_unchanged,
            target_mutated,
            false,
        );

        let dry_run_path = output_abs.join("dry_run_report.json");

        fs::write(&dry_run_path, serde_json::to_string_pretty(&dry_run)?)?;

        println!("\n==> [Gate 11.8] ISOLATED DRY RUN COMPLETE");
        println!("    Dry-run report: {}", dry_run_path.display());
        println!("    Candidate: {}", candidate_id);
        println!(
            "    Authoritative source unchanged: {}",
            if authoritative_source_unchanged { "YES" } else { "NO" }
        );
        println!("    Isolated target mutated: {}", if target_mutated { "YES" } else { "NO" });
        println!("    Promotion performed: NO");
        println!("    Human approval #2 required before promotion: YES");

        Ok(())
    }
}

/// Copy a workspace into an isolated directory.
///
/// Build artifacts and Git metadata are deliberately excluded.
fn copy_tree(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }

    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();

        // Skip heavy / non-source directories that would bloat the isolated
        // workspace and are irrelevant for the dry-run contract.
        if name == "target"
            || name == ".git"
            || name == "node_modules"
            || name == ".venv"
            || name == "venv"
            || name == "__pycache__"
            || name == "dist"
            || name == "build"
        {
            continue;
        }

        let destination_path = destination.join(&name);

        if path.is_dir() {
            copy_tree(&path, &destination_path)?;
        } else if path.is_file() {
            fs::copy(&path, &destination_path)?;
        }
    }

    Ok(())
}
