use crate::distributed_scheduler::DistributedScheduler;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::{MotorAction, MotorCommand};
use crate::shared_state::VmoState;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, PartialEq, Clone)]
pub enum FsmState {
    Idle,
    Executing,
    Complete,
    Fault,
}

pub struct RoboticExecutor {
    pub current_state: FsmState,
    pub api_key: String,
    #[allow(dead_code)]
    tx: tokio::sync::broadcast::Sender<String>,
    pub action_cache: Arc<tokio::sync::Mutex<crate::memory_cortex::ActionCache>>,
}

fn canonicalize_or_parent(p: &Path) -> Result<PathBuf> {
    if let Ok(c) = std::fs::canonicalize(p) {
        return Ok(c);
    }
    let mut curr = p.to_path_buf();
    let mut components = Vec::new();
    while !curr.exists() {
        if let Some(file_name) = curr.file_name() {
            components.push(file_name.to_os_string());
        }
        if let Some(parent) = curr.parent() {
            curr = parent.to_path_buf();
        } else {
            break;
        }
    }
    let ancestor_canon = std::fs::canonicalize(&curr)?;
    let mut safe_path = ancestor_canon;
    for part in components.into_iter().rev() {
        safe_path.push(part);
    }
    Ok(safe_path)
}

pub fn sanitize(path: &str) -> Result<PathBuf> {
    if path.starts_with("/tmp/") {
        return Ok(Path::new(path).to_path_buf());
    }

    let workspace_root = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .map_err(|e| anyhow!("PATH_TRAVERSAL_DENIED: could not resolve workspace root ({})", e))?;

    let mut normalized_path = path.to_string();

    if let Some(ws_str) = workspace_root.to_str() {
        if normalized_path.starts_with(ws_str) {
            normalized_path =
                normalized_path.trim_start_matches(ws_str).trim_start_matches('/').to_string();
        }
    }

    if normalized_path.starts_with("src/") {
        normalized_path = format!("crates/runtime_core/{}", normalized_path);
    } else if normalized_path.starts_with("runtime_core/")
        || normalized_path.starts_with("nexus_substrate/")
        || normalized_path.starts_with("nexus_model_hub/")
        || normalized_path.starts_with("nexus_search/")
        || normalized_path.starts_with("memory_graph/")
        || normalized_path.starts_with("protocol/")
        || normalized_path.starts_with("executive_cortex/")
    {
        normalized_path = format!("crates/{}", normalized_path);
    } else if normalized_path.starts_with("ide_backend/")
        || normalized_path.starts_with("cli/")
        || normalized_path.starts_with("node/")
    {
        normalized_path = format!("bins/{}", normalized_path);
    }

    let p = Path::new(&normalized_path);
    let target = if p.is_absolute() { p.to_path_buf() } else { workspace_root.join(p) };

    if !target.exists() {
        let crate_fallback = workspace_root.join("crates/runtime_core/src").join(p);
        if crate_fallback.exists() {
            return Ok(crate_fallback);
        }

        if let Some(filename) = p.file_name().and_then(|f| f.to_str()) {
            let lower = filename.to_lowercase();
            if let Ok(entries) = std::fs::read_dir(&workspace_root) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().to_lowercase() == lower {
                        return Ok(entry.path());
                    }
                }
            }
        }

        return Err(anyhow!(
            "PATH_NOT_FOUND: {} does not exist in the workspace (tried {}, crate-relative, and filename search)",
            path,
            target.display()
        ));
    }

    let canonical =
        canonicalize_or_parent(&target).map_err(|_| anyhow!("PATH_TRAVERSAL_DENIED: {}", path))?;

    if !canonical.starts_with(&workspace_root) {
        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {} resolves outside the workspace root", path));
    }

    Ok(canonical)
}

impl RoboticExecutor {
    pub fn new(api_key: String, tx: tokio::sync::broadcast::Sender<String>) -> Self {
        Self {
            current_state: FsmState::Idle,
            api_key,
            tx,
            action_cache: Arc::new(tokio::sync::Mutex::new(
                crate::memory_cortex::ActionCache::new(),
            )),
        }
    }

    pub async fn execute_motor_command(
        &mut self,
        command: MotorCommand,
        memory: Arc<MemoryCortex>,
        vmo_state: Arc<Mutex<VmoState>>,
        dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        self.execute_single_action(command.action, memory, vmo_state, dist).await
    }

    async fn execute_single_action(
        &mut self,
        action: MotorAction,
        memory: Arc<MemoryCortex>,
        _vmo_state: Arc<Mutex<VmoState>>,
        _dist: Arc<DistributedScheduler>,
    ) -> Result<String> {
        match action {
            MotorAction::ExecuteCommand { command } => {
                let cache_key =
                    crate::memory_cortex::ActionCache::generate_key(&command, "v7_workspace_ast");
                {
                    let cache = self.action_cache.lock().await;
                    if let Some(cached) = cache.get(&cache_key) {
                        return Ok(format!("[CACHE_HIT] (Cached execution)\n{}", cached.output));
                    }
                }

                let cmd_clone = command.clone();
                let exec_res = tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    tokio::task::spawn_blocking(move || {
                        crate::podman::PodmanShield::execute(&cmd_clone)
                    }),
                )
                .await;

                let out = match exec_res {
                    Ok(Ok(Ok(output))) => output,
                    Ok(Ok(Err(e))) => return Err(anyhow!("Command execution error: {}", e)),
                    Ok(Err(join_err)) => return Err(anyhow!("Task spawn error: {}", join_err)),
                    Err(_) => {
                        return Ok(format!("[TIMEOUT] Command timed out after 60s: `{}`", command));
                    }
                };
                let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

                let mut stdout_trunc = stdout;
                let mut stderr_trunc = stderr;
                if stdout_trunc.chars().count() > 2500 {
                    let safe_head: String = stdout_trunc.chars().take(2500).collect();
                    stdout_trunc =
                        format!("{}\n...[STDOUT TRUNCATED: OUTPUT TOO LARGE]", safe_head);
                }
                if stderr_trunc.chars().count() > 1500 {
                    let total_chars = stderr_trunc.chars().count();
                    let safe_tail: String =
                        stderr_trunc.chars().skip(total_chars.saturating_sub(1500)).collect();
                    stderr_trunc = format!("[... TRUNCATED TO SAVE TOKENS ...]\n{}", safe_tail);
                }

                let mut res = format!("[CMD_OK] Status: {}\n", out.status);
                if !stdout_trunc.trim().is_empty() {
                    res.push_str("STDOUT:\n");
                    res.push_str(&stdout_trunc);
                    res.push('\n');
                }
                if !stderr_trunc.trim().is_empty() {
                    res.push_str("STDERR:\n");
                    res.push_str(&stderr_trunc);
                    res.push('\n');
                }
                if stdout_trunc.trim().is_empty() && stderr_trunc.trim().is_empty() {
                    res.push_str("(Command produced no output)\n");
                }

                {
                    let mut cache = self.action_cache.lock().await;
                    cache.insert(cache_key, res.clone(), out.status.code().unwrap_or(0));
                }

                Ok(res)
            }
            MotorAction::WriteFile { path, content } => {
                let raw_path = Path::new(&path);

                let workspace_root =
                    std::env::current_dir().and_then(std::fs::canonicalize).map_err(|e| {
                        anyhow!("PATH_TRAVERSAL_DENIED: could not resolve workspace root ({})", e)
                    })?;

                let target = if raw_path.is_absolute() {
                    raw_path.to_path_buf()
                } else {
                    workspace_root.join(raw_path)
                };

                let safe_path = canonicalize_or_parent(&target)
                    .map_err(|_| anyhow!("PATH_TRAVERSAL_DENIED: {}", path))?;

                if !safe_path.starts_with(&workspace_root) {
                    return Err(anyhow!(
                        "PATH_TRAVERSAL_DENIED: {} resolves outside the workspace root",
                        path
                    ));
                }

                if let Ok(existing) = tokio::fs::read_to_string(&safe_path).await {
                    if existing == content {
                        return Ok(format!(
                            "[WRITE_NOOP] {} -- content identical to existing file",
                            path
                        ));
                    }
                }
                if let Some(parent) = safe_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                tokio::fs::write(&safe_path, &content).await?;
                _dist.broadcast_file_shift(&path, "write_file");
                *self.action_cache.lock().await = crate::memory_cortex::ActionCache::new();

                let snippet: String = content.chars().take(4000).collect();
                let emb = nexus_substrate::vector_bridge::get_embedding(&self.api_key, &snippet)
                    .await
                    .ok();
                let _ = memory.store_memory("WriteFile", &path, &content, "Mutated", emb).await;

                Ok(format!("[WRITE_OK] {}", path))
            }
            MotorAction::ReadFile { path } => {
                let safe_path = sanitize(&path)?;
                let content = tokio::fs::read_to_string(&safe_path).await?;
                let snippet: String = content.chars().take(4000).collect();
                if let Ok(emb) =
                    nexus_substrate::vector_bridge::get_embedding(&self.api_key, &snippet).await
                {
                    let _ = memory
                        .store_memory("ReadFile", &path, &content, "Observed", Some(emb))
                        .await;
                } else {
                    let _ =
                        memory.store_memory("ReadFile", &path, &content, "Observed", None).await;
                }

                let total_lines = content.lines().count();
                let return_content = if content.chars().count() > 3000 {
                    let prefix: String = content.chars().take(3000).collect();
                    format!("{}...\n\n[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: {}. USE THE `read_file_chunk` TOOL BY SPECIFYING `start_line` AND `max_lines` TO READ BEYOND THIS POINT.]", prefix, total_lines)
                } else {
                    content.clone()
                };

                Ok(format!("[READ_OK] {}\nFULL FILE CONTENT:\n{}", path, return_content))
            }
            MotorAction::ReadFunction { path, function_name } => {
                let safe_path = sanitize(&path)?;

                let source = nexus_ast_tool::agent::RefactorAgent::read_function_source(
                    &safe_path,
                    &function_name,
                )?;

                let snippet: String = source.chars().take(4000).collect();

                if let Ok(emb) =
                    nexus_substrate::vector_bridge::get_embedding(&self.api_key, &snippet).await
                {
                    let _ = memory
                        .store_memory(
                            "ReadFunction",
                            &format!("{}:{}", path, function_name),
                            &source,
                            "Observed",
                            Some(emb),
                        )
                        .await;
                } else {
                    let _ = memory
                        .store_memory(
                            "ReadFunction",
                            &format!("{}:{}", path, function_name),
                            &source,
                            "Observed",
                            None,
                        )
                        .await;
                }

                Ok(format!(
                    "[READ_FUNCTION_OK] File: {} | Function: {}\nEXACT AST SOURCE:\n{}",
                    path, function_name, source
                ))
            }

            MotorAction::ReadFileChunk { path, start_line, max_lines } => {
                let safe_path = sanitize(&path)?;
                let content = tokio::fs::read_to_string(&safe_path).await?;
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                if start_line >= total_lines {
                    return Ok(format!(
                        "[READ_CHUNK_EOF] Reached end of file {}. Total lines: {}",
                        path, total_lines
                    ));
                }

                let end_line = std::cmp::min(start_line + max_lines, total_lines);
                let chunk = lines[start_line..end_line].join("\n");

                Ok(format!(
                    "[READ_CHUNK_OK] File: {} | Lines {}-{} of {}\nCONTENT:\n{}",
                    path, start_line, end_line, total_lines, chunk
                ))
            }
            MotorAction::SearchCode { path, query } => {
                if query.trim().is_empty() {
                    return Ok("[SEARCH_FAILED] Empty search query is not permitted. Provide a specific search symbol or pattern.".to_string());
                }

                let safe_path = sanitize(&path).unwrap_or_else(|_| PathBuf::from("crates"));
                let p_str = safe_path.to_str().unwrap_or(&path);

                let cmd = format!(
                    "(rg -n -C 2 -i '{}' {} || grep -rn -C 2 --exclude-dir={{.git,target,.surgical_backup,.nexus,.lazarus,node_modules}} -iE '{}' {}) 2>/dev/null || true",
                    query, p_str, query, p_str
                );
                let out = crate::podman::PodmanShield::execute(&cmd)?;
                let mut stdout = String::from_utf8_lossy(&out.stdout).to_string();

                if stdout.trim().is_empty()
                    && (p_str.contains("executive_cortex") || query.contains("router"))
                {
                    let fallback_cmd = format!(
                        "(rg -n -C 2 -i '{}' crates/runtime_core/src/ || grep -rn -C 2 --exclude-dir={{.git,target,.surgical_backup,.nexus,.lazarus,node_modules}} -iE '{}' crates/runtime_core/src/) 2>/dev/null || true",
                        query, query
                    );
                    if let Ok(fb_out) = crate::podman::PodmanShield::execute(&fallback_cmd) {
                        let fb_stdout = String::from_utf8_lossy(&fb_out.stdout);
                        if !fb_stdout.trim().is_empty() {
                            stdout = format!(
                                "[FALLBACK MATCHES FROM crates/runtime_core/src/]:\n{}",
                                fb_stdout
                            );
                        }
                    }
                }

                let trunc = if stdout.chars().count() > 2500 {
                    let prefix: String = stdout.chars().take(2500).collect();
                    format!("{}...\n[MATCHES TRUNCATED: OUTPUT TOO LARGE]", prefix)
                } else {
                    stdout
                };

                if trunc.trim().is_empty() {
                    Ok(format!("[SEARCH_CODE_OK] Path: {} | Query: '{}'\nNo code matches found in specified path.", path, query))
                } else {
                    Ok(format!(
                        "[SEARCH_CODE_OK] Path: {} | Query: '{}'\nMATCHES:\n{}",
                        path, query, trunc
                    ))
                }
            }
            MotorAction::ReplaceLines { path, start_line, end_line, replacement } => {
                let safe_path = sanitize(&path)?;
                let content = tokio::fs::read_to_string(&safe_path).await?;
                let lines: Vec<&str> = content.split('\n').collect();
                let start_idx = start_line.saturating_sub(1);
                let end_idx = end_line.saturating_sub(1);

                if start_idx > lines.len() || start_idx > end_idx {
                    return Err(anyhow::anyhow!("Invalid line range for replacement"));
                }
                let end_idx_actual = std::cmp::min(end_idx, lines.len().saturating_sub(1));

                let before = lines[..start_idx].to_vec();
                let after = if end_idx_actual + 1 < lines.len() {
                    lines[end_idx_actual + 1..].to_vec()
                } else {
                    vec![]
                };

                let mut new_lines = before;
                new_lines.push(replacement.as_str());
                new_lines.extend(after);

                tokio::fs::write(&safe_path, new_lines.join("\n")).await?;
                Ok(format!(
                    "[REPLACE_LINES_OK] Replaced lines {}-{} in {}",
                    start_line, end_line, path
                ))
            }
            MotorAction::RunTests { test_target } => {
                let cmd = if test_target.is_empty() || test_target == "." {
                    "cargo test --workspace --color never 2>&1".to_string()
                } else {
                    format!("cargo test {} --color never 2>&1", test_target)
                };

                let out = crate::podman::PodmanShield::execute(&cmd).unwrap_or_else(|e| {
                    std::process::Output {
                        status: std::os::unix::process::ExitStatusExt::from_raw(1),
                        stdout: Vec::new(),
                        stderr: format!("TEST HARNESS FAILED: {}", e).into_bytes(),
                    }
                });

                let output_str = String::from_utf8_lossy(&out.stdout);
                let err_str = String::from_utf8_lossy(&out.stderr);
                let combined = format!("STDOUT:\n{}\nSTDERR:\n{}", output_str, err_str);

                let char_count = combined.chars().count();
                let trunc = if char_count > 3000 {
                    let safe_tail: String =
                        combined.chars().skip(char_count.saturating_sub(3000)).collect();
                    format!("{}...\n[OUTPUT TRUNCATED]", safe_tail)
                } else {
                    combined
                };

                if out.status.success() {
                    Ok(format!("[TEST_OK] Tests passed for target '{}':\n{}", test_target, trunc))
                } else {
                    Ok(format!(
                        "[TEST_FAILED] Tests failed for target '{}':\n{}",
                        test_target, trunc
                    ))
                }
            }
            MotorAction::AtomicPatch { patches } => {
                let mut patched_files = Vec::new();
                for patch in patches {
                    let safe_path = sanitize(&patch.path)?;
                    let original = tokio::fs::read_to_string(&safe_path).await?;
                    let patched =
                        crate::diff::DiffEngine::apply(&original, &patch.search, &patch.replace)?;
                    tokio::fs::write(&safe_path, patched).await?;
                    patched_files.push(patch.path);
                }
                Ok(format!(
                    "[ATOMIC_PATCH_OK] Successfully patched {} files: {:?}",
                    patched_files.len(),
                    patched_files
                ))
            }
            MotorAction::DiffPatch { path, search, replace } => {
                let safe_path = sanitize(&path)?;
                let original = tokio::fs::read_to_string(&safe_path).await?;
                let patched = crate::diff::DiffEngine::apply(&original, &search, &replace)?;
                tokio::fs::write(&safe_path, &patched).await?;
                _dist.broadcast_file_shift(&path, "diff_patch");
                *self.action_cache.lock().await = crate::memory_cortex::ActionCache::new();

                let snippet: String = patched.chars().take(4000).collect();
                let emb = nexus_substrate::vector_bridge::get_embedding(&self.api_key, &snippet)
                    .await
                    .ok();
                let _ = memory.store_memory("DiffPatch", &path, &patched, "Mutated", emb).await;

                Ok(format!("[DIFF_PATCH_OK] Applied changes to {}", path))
            }
            MotorAction::PatchFunction { path, function_name, new_body } => {
                let safe_path = sanitize(&path)?;
                let patched = crate::ast_editor::AstEditor::update_function(
                    safe_path.to_str().unwrap_or(&path),
                    &function_name,
                    &new_body,
                )?;
                tokio::fs::write(&safe_path, patched).await?;
                Ok(format!("[PATCH_FUNCTION_OK] Updated function '{}' in {}", function_name, path))
            }
            MotorAction::SearchMemory { query } => {
                let emb = nexus_substrate::vector_bridge::get_embedding(&self.api_key, &query)
                    .await
                    .map_err(|e| anyhow!("SEMANTIC_SEARCH_EMBEDDING_FAILED: {}", e))?;

                let results = memory
                    .semantic_search(&emb, 3)
                    .await
                    .map_err(|e| anyhow!("SEMANTIC_SEARCH_VECTOR_RECALL_FAILED: {}", e))?;

                if results.is_empty() {
                    return Ok(format!(
                        "[SEARCH_MEMORY_EMPTY] Semantic search completed successfully but returned zero matches for: '{}'",
                        query
                    ));
                }

                // Semantic retrieval supplies the candidates. Apply a lightweight,
                // intent-aware rerank so factual host-observation queries prefer
                // actual metric observations over reflection/failure memories.
                let query_lower = query.to_lowercase();
                let host_terms = [
                    "uptime",
                    "disk",
                    "filesystem",
                    "available",
                    "memory",
                    "ram",
                    "load average",
                ];

                let host_term_hits = host_terms
                    .iter()
                    .filter(|term| query_lower.contains(**term))
                    .count();

                let host_observation_query = host_term_hits >= 2;

                let mut ranked_results: Vec<_> =
                    results
                        .into_iter()
                        .enumerate()
                        .map(|(original_order, r)| {
                            let score = if !host_observation_query {
                                0
                            } else {
                                let content = format!(
                                    "{} {}",
                                    r.directive.to_lowercase(),
                                    r.result.to_lowercase()
                                );

                                let term_hits = host_terms
                                    .iter()
                                    .filter(|term| content.contains(**term))
                                    .count();

                                let factual_origin = matches!(
                                    r.origin.to_lowercase().as_str(),
                                    "readfile" | "executecommand" | "execute_command"
                                );

                                let reflection_memory =
                                    r.origin.eq_ignore_ascii_case("ReflectionEngine");

                                let actual_metric_output = [
                                    "filesystem",
                                    "mem:",
                                    "load average",
                                    "total used free",
                                    " up ",
                                ]
                                .iter()
                                .any(|marker| content.contains(marker));

                                (term_hits as i32 * 2)
                                    + if factual_origin { 3 } else { 0 }
                                    + if actual_metric_output { 3 } else { 0 }
                                    - if reflection_memory { 5 } else { 0 }
                            };

                            (score, original_order, r)
                        })
                        .collect();

                ranked_results.sort_by(|a, b| {
                    b.0.cmp(&a.0)
                        .then_with(|| a.1.cmp(&b.1))
                });

                let mut results_text = String::new();

                for (_, _, r) in ranked_results.into_iter().take(3) {
                    let snippet: String = r.result.chars().take(1500).collect();

                    results_text
                        .push_str(&format!("Source: {}\nData: {}...\n---\n", r.directive, snippet));
                }

                Ok(format!("[SEARCH_MEMORY_OK] Found semantic context:\n{}", results_text))
            }
            MotorAction::ScrapeUrl { url, prompt } => {
                let lower = url.to_lowercase();

                if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                    return Err(anyhow!("SCRAPE_URL_REJECTED: URL must use http:// or https://"));
                }

                let host_part = if lower.starts_with("https://") { &url[8..] } else { &url[7..] };

                if host_part.trim().is_empty() {
                    return Err(anyhow!("SCRAPE_URL_REJECTED: URL has no host"));
                }

                if prompt.trim().is_empty() {
                    return Err(anyhow!("SCRAPE_URL_REJECTED: extraction prompt is empty"));
                }

                let workspace_root = std::env::current_dir().map_err(|e| {
                    anyhow!("SCRAPE_EXECUTOR_ERROR: cannot resolve workspace: {}", e)
                })?;

                let binary = workspace_root.join("target/release/nexus-scrape");

                let metadata = tokio::fs::metadata(&binary).await.map_err(|e| {
                    anyhow!(
                        "SCRAPE_EXECUTOR_ERROR: nexus_scrape binary unavailable at {}: {}",
                        binary.display(),
                        e
                    )
                })?;

                if !metadata.is_file() {
                    return Err(anyhow!(
                        "SCRAPE_EXECUTOR_ERROR: nexus_scrape path is not a regular file: {}",
                        binary.display()
                    ));
                }

                let output = tokio::time::timeout(
                    std::time::Duration::from_secs(120),
                    tokio::process::Command::new(&binary)
                        .arg(&url)
                        .arg("--extract")
                        .arg(&prompt)
                        .output(),
                )
                .await
                .map_err(|_| {
                    anyhow!("SCRAPE_TIMEOUT: nexus_scrape exceeded 120 seconds for {}", url)
                })?
                .map_err(|e| {
                    anyhow!("SCRAPE_EXECUTOR_ERROR: failed to launch nexus_scrape: {}", e)
                })?;

                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

                if !output.status.success() {
                    let stderr_tail = {
                        let count = stderr.chars().count();
                        if count > 2500 {
                            stderr.chars().skip(count.saturating_sub(2500)).collect::<String>()
                        } else {
                            stderr.clone()
                        }
                    };

                    return Err(anyhow!(
                        "SCRAPE_FAILED: nexus_scrape exited with {}\nSTDERR:\n{}",
                        output.status,
                        stderr_tail
                    ));
                }

                let extracted = stdout.trim();

                if extracted.is_empty() {
                    return Err(anyhow!(
                        "SCRAPE_VALIDATION_FAILED: nexus_scrape returned empty stdout"
                    ));
                }

                // Bound persisted external content so a pathological page/extractor
                // response cannot create unbounded memory or embedding work.
                let persisted: String = extracted.chars().take(100_000).collect();

                let embedding_text: String = persisted.chars().take(8_000).collect();

                let embedding =
                    nexus_substrate::vector_bridge::get_embedding(&self.api_key, &embedding_text)
                        .await
                        .ok();

                memory
                    .store_memory("WebScrape", &url, &persisted, "Untrusted", embedding)
                    .await
                    .map_err(|e| anyhow!("SCRAPE_MEMORY_STORE_FAILED: {}", e))?;

                let evidence: String = persisted.chars().take(5000).collect();

                Ok(format!(
                    "[SCRAPE_OK] URL: {}\n[SCRAPE_TAINT: Untrusted]\nEXTRACTION:\n{}",
                    url, evidence
                ))
            }
            MotorAction::PatchFile { path, target, replacement } => {
                let safe_path = sanitize(&path)?;
                let file_content = tokio::fs::read_to_string(&safe_path)
                    .await
                    .map_err(|e| anyhow!("Failed to read file: {}", e))?;
                if !file_content.contains(&target) {
                    return Err(anyhow!("Target string not found in file: {}", target));
                }
                let new_content = file_content.replace(&target, &replacement);
                tokio::fs::write(&safe_path, new_content)
                    .await
                    .map_err(|e| anyhow!("Failed to write file: {}", e))?;
                Ok(format!("Successfully patched file: {}", path))
            }
            MotorAction::SynthesizeReport { output_path, source_files } => {
                let mut combined_context = String::new();
                for file in source_files {
                    if let Ok(safe_p) = sanitize(&file) {
                        if let Ok(content) = tokio::fs::read_to_string(&safe_p).await {
                            combined_context
                                .push_str(&format!("\n--- FILE: {} ---\n{}\n", file, content));
                        }
                    }
                }

                let prompt = format!("You are a Synthesis Subagent. Analyze the following source code files and output a comprehensive architectural markdown report. DO NOT output JSON. Output raw markdown only.\n\nFILES:\n{}", combined_context);

                let payload = serde_json::json!({
                    "contents": [{"role": "user", "parts": [{"text": prompt}]}],
                    "generationConfig": { "temperature": 0.2, "maxOutputTokens": 8192 }
                });

                let heavy_models = crate::model_config::get_heavy_models();
                let client = reqwest::Client::new();

                match crate::epistemic_engine::EpistemicEngine::safe_api_call(
                    &client,
                    &self.api_key,
                    &heavy_models,
                    &payload,
                )
                .await
                {
                    Ok(markdown) => {
                        let raw_output_path = Path::new(&output_path);
                        let workspace_root = std::env::current_dir()
                            .and_then(std::fs::canonicalize)
                            .map_err(|e| {
                                anyhow!(
                                    "PATH_TRAVERSAL_DENIED: could not resolve workspace root ({})",
                                    e
                                )
                            })?;
                        let output_target = if raw_output_path.is_absolute() {
                            raw_output_path.to_path_buf()
                        } else {
                            workspace_root.join(raw_output_path)
                        };
                        let safe_path = canonicalize_or_parent(&output_target)
                            .map_err(|_| anyhow!("PATH_TRAVERSAL_DENIED: {}", output_path))?;
                        if !safe_path.starts_with(&workspace_root) {
                            return Err(anyhow!(
                                "PATH_TRAVERSAL_DENIED: {} resolves outside the workspace root",
                                output_path
                            ));
                        }

                        if let Some(parent) = safe_path.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        tokio::fs::write(&safe_path, markdown).await?;
                        _dist.broadcast_file_shift(&output_path, "synthesize_report");
                        *self.action_cache.lock().await = crate::memory_cortex::ActionCache::new();
                        Ok(format!(
                            "[WRITE_OK] Successfully synthesized and wrote report to {}",
                            output_path
                        ))
                    }
                    Err(e) => Err(anyhow::anyhow!("Synthesis Failed: {}", e)),
                }
            }
            MotorAction::Halt { reasoning } => Ok(format!("[HALT] {}", reasoning)),
            MotorAction::PolyglotAstPatch { .. } => {
                Err(anyhow!("PolyglotAstPatch is defined but not yet implemented in the executor"))
            }
        }
    }
}
