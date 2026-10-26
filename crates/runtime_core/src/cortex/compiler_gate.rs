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

        // Cache cargo metadata per manifest so multi-file mutations in the
        // same nested workspace only shell out once.
        let mut meta_cache: std::collections::HashMap<
            PathBuf,
            (PathBuf, Vec<serde_json::Value>),
        > = std::collections::HashMap::new();

        let root_meta = Self::load_workspace_metadata(&workspace_manifest).await?;
        meta_cache.insert(workspace_manifest.clone(), root_meta.clone());

        let mut matched: Vec<(PathBuf, String)> = Vec::new(); // (manifest, package)
        let mut standalone_files: Vec<PathBuf> = Vec::new();

        for mutated_path in &rs_paths {
            let absolute_path = if mutated_path.is_absolute() {
                mutated_path.clone()
            } else {
                root_meta.0.join(mutated_path)
            };

            // Pass 1: parent workspace.
            if let Some(pkg) = Self::find_owner(&root_meta.1, &absolute_path) {
                matched.push((workspace_manifest.clone(), pkg));
                continue;
            }

            // Pass 2: walk up for the nearest Cargo.toml that isn't the
            // parent workspace's. This is the fix for vendor/<nested>/...
            // paths that were previously misclassified as standalone.
            let mut resolved = false;
            if let Some(nested_manifest) =
                Self::nearest_cargo_toml(&absolute_path, &workspace_manifest)
            {
                let cached = if let Some(pair) = meta_cache.get(&nested_manifest) {
                    Some(pair.clone())
                } else {
                    match Self::load_workspace_metadata(&nested_manifest).await {
                        Ok(pair) => {
                            meta_cache.insert(nested_manifest.clone(), pair.clone());
                            Some(pair)
                        }
                        Err(_) => None,
                    }
                };
                if let Some((_, packages)) = cached {
                    if let Some(pkg) = Self::find_owner(&packages, &absolute_path) {
                        matched.push((nested_manifest, pkg));
                        resolved = true;
                    }
                }
            }
            if !resolved {
                standalone_files.push(mutated_path.clone());
            }
        }

        // The current enum shape carries one workspace manifest. If the
        // mutation touches files in multiple workspaces, fail loudly so the
        // caller splits the mutation rather than silently checking one.
        let unique_manifests: std::collections::HashSet<&PathBuf> =
            matched.iter().map(|(m, _)| m).collect();
        if unique_manifests.len() > 1 {
            return Err(format!(
                "mutation spans {} workspaces ({}); split so each workspace is verified independently",
                unique_manifests.len(),
                unique_manifests
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if let Some((manifest, _)) = matched.first() {
            let manifest = manifest.clone();
            let packages: Vec<String> =
                matched.into_iter().map(|(_, pkg)| pkg).collect();
            Ok(RustVerificationTarget::WorkspacePackages {
                workspace_manifest: manifest,
                packages,
            })
        } else if !standalone_files.is_empty() {
            Ok(RustVerificationTarget::StandaloneFiles { files: standalone_files })
        } else {
            Ok(RustVerificationTarget::NoVerificationNeeded)
        }
    }

    /// Load cargo metadata for a specific manifest. Returns
    /// `(workspace_root, packages_array)`.
    async fn load_workspace_metadata(
        manifest: &Path,
    ) -> Result<(PathBuf, Vec<serde_json::Value>), String> {
        let output = tokio::process::Command::new("cargo")
            .args(["metadata", "--no-deps", "--format-version=1", "--manifest-path"])
            .arg(manifest)
            .output()
            .await
            .map_err(|e| format!("cargo metadata failed: {e}"))?;

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

        let root = meta
            .get("workspace_root")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .ok_or_else(|| "cargo metadata omitted workspace_root".to_string())?;

        let packages = meta
            .get("packages")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| "cargo metadata omitted packages".to_string())?;

        Ok((root, packages))
    }

    /// Package name whose manifest directory contains `abs_path`.
    fn find_owner(packages: &[serde_json::Value], abs_path: &Path) -> Option<String> {
        for package in packages {
            let Some(name) = package.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(manifest_path) = package.get("manifest_path").and_then(|v| v.as_str())
            else {
                continue;
            };
            let Ok(package_root) = Path::new(manifest_path).parent().ok_or("invalid path")
            else {
                continue;
            };
            if abs_path.starts_with(package_root) {
                return Some(name.to_string());
            }
        }
        None
    }

    /// Nearest Cargo.toml above `abs_path`, excluding the parent workspace's
    /// own manifest.
    fn nearest_cargo_toml(abs_path: &Path, parent_manifest: &Path) -> Option<PathBuf> {
        let mut cur = abs_path.parent()?;
        loop {
            let candidate = cur.join("Cargo.toml");
            if candidate.exists() && candidate != parent_manifest {
                return Some(candidate);
            }
            cur = cur.parent()?;
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
                    // rustc creates its temp files next to its output. With
                    // `-o /dev/null` it tries to mkdir inside /dev, which fails
                    // under proot ("couldn't create a temp dir: Permission
                    // denied"), so every standalone check reported a compile
                    // failure for valid code. Give it a real scratch directory.
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    let scratch = std::env::temp_dir().join(format!(
                        "nexus_rustc_{}_{}",
                        std::process::id(),
                        nanos
                    ));
                    if let Err(e) = std::fs::create_dir_all(&scratch) {
                        return CompilerGateResult::ExecutionFault {
                            error_message: format!(
                                "Failed to create rustc scratch dir {}: {}",
                                scratch.display(),
                                e
                            ),
                        };
                    }
                    let out = tokio::process::Command::new("rustc")
                        .args(["--edition", "2021", "--crate-type=lib", "--emit=metadata"])
                        .arg("--out-dir")
                        .arg(&scratch)
                        .arg(file)
                        .output()
                        .await;
                    let _ = std::fs::remove_dir_all(&scratch);

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
