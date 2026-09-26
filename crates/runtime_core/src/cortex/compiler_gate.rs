use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum RustVerificationTarget {
    WorkspacePackages { workspace_manifest: PathBuf, packages: Vec<String> },
    StandaloneFiles { files: Vec<PathBuf> },
    NoVerificationNeeded,
}

pub enum CompilerGateResult {
    Success { success_marker: String, targets_checked: Vec<String> },
    CompilationFailed { error_preview: String, failed_targets: Vec<String> },
    ExecutionFault { error_message: String },
}

pub struct CompilerGate;

impl CompilerGate {
    /// Inspects mutated paths and authoritative cargo metadata to classify
    /// whether mutations affect workspace packages, standalone files, or require no check.
    pub async fn resolve_target(
        mutation_paths: &[String],
    ) -> Result<RustVerificationTarget, String> {
        let rs_paths: Vec<PathBuf> =
            mutation_paths.iter().filter(|p| p.ends_with(".rs")).map(PathBuf::from).collect();

        if rs_paths.is_empty() {
            return Ok(RustVerificationTarget::NoVerificationNeeded);
        }

        let workspace_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("Cargo.toml"))
            .ok_or_else(|| {
                "unable to derive workspace Cargo.toml from CARGO_MANIFEST_DIR".to_string()
            })?;

        if !workspace_manifest.exists() {
            return Err(format!(
                "workspace Cargo.toml does not exist: {}",
                workspace_manifest.display()
            ));
        }

        let output = tokio::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version=1", "--manifest-path"])
            .arg(&workspace_manifest)
            .output()
            .await
            .map_err(|e| format!("cargo metadata failed to execute: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let preview = stderr.lines().take(20).collect::<Vec<_>>().join("\n");
            return Err(format!(
                "cargo metadata exited non-zero ({}):\n{}",
                output.status, preview
            ));
        }

        let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("cargo metadata JSON parse failed: {e}"))?;

        let workspace_root = meta
            .get("workspace_root")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .ok_or_else(|| "cargo metadata omitted workspace_root".to_string())?;

        let packages = meta
            .get("packages")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "cargo metadata omitted packages".to_string())?;

        let mut matched_packages = Vec::new();
        let mut standalone_files = Vec::new();

        for mutated_path in &rs_paths {
            let absolute_path = if mutated_path.is_absolute() {
                mutated_path.clone()
            } else {
                workspace_root.join(mutated_path)
            };

            let mut owner = None;
            for package in packages {
                let Some(name) = package.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(manifest_path) = package.get("manifest_path").and_then(|v| v.as_str())
                else {
                    continue;
                };
                let Ok(package_root) =
                    Path::new(manifest_path).parent().ok_or_else(|| "invalid path")
                else {
                    continue;
                };

                if absolute_path.starts_with(package_root) {
                    owner = Some(name.to_string());
                    break;
                }
            }

            if let Some(pkg) = owner {
                if !matched_packages.contains(&pkg) {
                    matched_packages.push(pkg);
                }
            } else {
                standalone_files.push(mutated_path.clone());
            }
        }

        if !matched_packages.is_empty() {
            Ok(RustVerificationTarget::WorkspacePackages {
                workspace_manifest,
                packages: matched_packages,
            })
        } else if !standalone_files.is_empty() {
            Ok(RustVerificationTarget::StandaloneFiles { files: standalone_files })
        } else {
            Ok(RustVerificationTarget::NoVerificationNeeded)
        }
    }

    /// Run verification against the resolved target.
    pub async fn verify_target(target: &RustVerificationTarget) -> CompilerGateResult {
        match target {
            RustVerificationTarget::WorkspacePackages { workspace_manifest, packages } => {
                let mut cmd = tokio::process::Command::new("cargo");
                cmd.arg("check").arg("--manifest-path").arg(workspace_manifest);
                for pkg in packages {
                    cmd.arg("-p").arg(pkg);
                }

                match cmd.output().await {
                    Ok(out) if out.status.success() => CompilerGateResult::Success {
                        success_marker: format!(
                            "[COMPILE_OK] Package(s) {} compile successfully.\n",
                            packages.join(", ")
                        ),
                        targets_checked: packages.clone(),
                    },
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let err_preview = stdout
                            .lines()
                            .chain(stderr.lines())
                            .take(15)
                            .collect::<Vec<_>>()
                            .join("\n");
                        CompilerGateResult::CompilationFailed {
                            error_preview: err_preview,
                            failed_targets: packages.clone(),
                        }
                    }
                    Err(e) => CompilerGateResult::ExecutionFault {
                        error_message: format!("Failed to execute cargo check: {}", e),
                    },
                }
            }
            RustVerificationTarget::StandaloneFiles { files } => {
                let mut failed_files = Vec::new();
                let mut error_preview = String::new();

                for file in files {
                    let out = tokio::process::Command::new("rustc")
                        .args(["--crate-type=lib", "--emit=metadata", "-o", "/dev/null"])
                        .arg(file)
                        .output()
                        .await;

                    match out {
                        Ok(o) if !o.status.success() => {
                            failed_files.push(file.display().to_string());
                            let stderr = String::from_utf8_lossy(&o.stderr);
                            error_preview
                                .push_str(&stderr.lines().take(15).collect::<Vec<_>>().join("\n"));
                            error_preview.push('\n');
                        }
                        Err(e) => {
                            return CompilerGateResult::ExecutionFault {
                                error_message: format!(
                                    "Failed to spawn rustc for {}: {}",
                                    file.display(),
                                    e
                                ),
                            };
                        }
                        _ => {}
                    }
                }

                if failed_files.is_empty() {
                    let names: Vec<String> =
                        files.iter().map(|f| f.display().to_string()).collect();
                    CompilerGateResult::Success {
                        success_marker: format!(
                            "[COMPILE_OK] Standalone Rust file(s) {} verified cleanly.\n",
                            names.join(", ")
                        ),
                        targets_checked: names,
                    }
                } else {
                    CompilerGateResult::CompilationFailed {
                        error_preview,
                        failed_targets: failed_files,
                    }
                }
            }
            RustVerificationTarget::NoVerificationNeeded => CompilerGateResult::Success {
                success_marker: String::new(),
                targets_checked: Vec::new(),
            },
        }
    }
}
