use crate::distributed_scheduler::DistributedScheduler;
use crate::memory_cortex::MemoryCortex;
use crate::motor_cortex::{MotorAction, MotorCommand};
use crate::shared_state::VmoState;
use anyhow::{anyhow, Result};
use nexus_substrate::episode::EpisodeContext;
use nexus_substrate::taint::TaintLabel;
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
    /// Episode provenance attached to every memory write this
    /// executor performs. Defaults to all-None ("episode
    /// unknown"), which the automatic-recall gate treats as
    /// foreign. Set by the caller before execution begins.
    pub episode: EpisodeContext,
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

/// Best-effort detection of a shell command's write target for `>`/`>>` file
/// redirection (including heredoc forms like `cat > path << 'EOF'`). Used to
/// synthesize a [WRITE_OK] marker for ExecuteCommand actions that mutate the
/// filesystem via shell redirection instead of a native WriteFile action --
/// previously these produced only [CMD_OK], which the deterministic
/// completion contract (outcome_verifier.rs) never recognizes as satisfying
/// a WriteMarkerAcquired condition, causing correct writes to loop forever
/// on rejected HALT proposals.
///
/// Heuristic and conservative: only fires on a literal `>`/`>>` token with a
/// following non-empty, non-fd-duplication target that `sanitize()` confirms
/// actually exists inside the workspace root. Call this only after the
/// command has already executed successfully, since sanitize() requires the
/// path to exist.
pub fn detect_shell_write_target(command: &str) -> Option<String> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    for (i, tok) in tokens.iter().enumerate() {
        if *tok == ">" || *tok == ">>" {
            let Some(target) = tokens.get(i + 1) else {
                continue;
            };
            if target.starts_with('&') {
                continue;
            }
            let cleaned = target.trim_matches('\\').trim_matches('"');
            if cleaned.is_empty() {
                continue;
            }
            if sanitize(cleaned).is_ok() {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

/// Test-only fault-injection hooks for the iterative compile-remediation
/// path.
///
/// These functions are inert unless `NEXUS_INJECT_COMPILE_FAIL_ONCE=1` is
/// set in the daemon environment. When enabled, the FIRST successful
/// `PatchFunction` in this process has its just-computed `patched` source
/// corrupted in-memory (never on disk, never in the memory store) before
/// the single canonical write. The corrupted source is syntactically
/// valid Rust that fails type-checking, so the package-scoped compiler
/// critic observes a real compile failure and can exercise the retained-
/// retry path deterministically.
///
/// The one-shot flag is a process-global `AtomicBool`. It is consumed
/// only after `ReplaceFunctionBody::apply_for_path` has produced a
/// byte-different source from `patched`; if AST mutation fails or
/// produces identical output, the flag is left untouched so a later
/// `PatchFunction` can still inject.
fn maybe_injected_source(
    safe_path: &Path,
    patched: &str,
    function_name: &str,
    new_body: &str,
) -> Option<String> {
    let enabled =
        std::env::var("NEXUS_INJECT_COMPILE_FAIL_ONCE").map(|v| v == "1").unwrap_or(false);
    if !enabled {
        return None;
    }
    try_one_shot_fault_injection(safe_path, patched, function_name, new_body)
}

fn try_one_shot_fault_injection(
    safe_path: &Path,
    patched: &str,
    function_name: &str,
    new_body: &str,
) -> Option<String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CONSUMED: AtomicBool = AtomicBool::new(false);

    if CONSUMED.load(Ordering::SeqCst) {
        return None;
    }

    // The injection payload is derived from the LLM's own `new_body` by
    // inserting a single type-mismatched `let` at the top of the body.
    // This preserves whatever the model actually wrote below the marker,
    // so the corrupted source is as close as possible to the intended
    // patch. The anchor is the first `{` in `new_body`; the pipeline
    // convention is that `new_body` is a full function definition, so the
    // first `{` is the body opening brace. If `new_body` has no `{`, we
    // do not consume the one-shot — a bare body payload is not a shape
    // the pipeline produces, and silently corrupting it would be wrong.
    if !new_body.contains('{') {
        tracing::warn!(
            "[FAULT INJECTION] new_body for {}:{} contains no `{{`; not consuming one-shot",
            safe_path.display(),
            function_name,
        );
        return None;
    }

    let corrupted_payload = new_body.replacen(
        '{',
        "{ let __nexus_fault_injection_marker: i32 = \"fault injection\";",
        1,
    );

    let corrupted = match nexus_ast_tool::mutation::ReplaceFunctionBody::apply_for_path(
        safe_path,
        patched,
        function_name,
        &corrupted_payload,
    ) {
        Ok(c) if c != patched => c,
        Ok(_) => {
            tracing::warn!(
                "[FAULT INJECTION] apply_for_path produced identical source for {}:{}; not consuming one-shot",
                safe_path.display(),
                function_name,
            );
            return None;
        }
        Err(e) => {
            tracing::warn!(
                "[FAULT INJECTION] apply_for_path rejected injection for {}:{}: {}; not consuming one-shot",
                safe_path.display(),
                function_name,
                e,
            );
            return None;
        }
    };

    // Only now, with a proven, distinct corrupted source in hand, do we
    // atomically consume the one-shot. If another caller won the race,
    // that caller's injection is used and ours is discarded.
    match CONSUMED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(_) => {
            tracing::warn!(
                "[FAULT INJECTION] one-shot compile-error injection armed for {}:{}",
                safe_path.display(),
                function_name,
            );
            Some(corrupted)
        }
        Err(_) => None,
    }
}

/// Rejects queries that cannot be searched safely or unambiguously.
/// Extracted so executor tests can exercise the guard without
/// constructing a full RoboticExecutor (which needs episode state,
/// broadcast channel, and action cache).
pub(crate) fn validate_search_query(query: &str) -> anyhow::Result<()> {
    if query.trim().is_empty() || query.contains('\n') || query.contains('\r') {
        return Err(anyhow::anyhow!(
            "SEARCH_FAILED: query must be non-empty and contain no line breaks"
        ));
    }
    Ok(())
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
            episode: EpisodeContext::default(),
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
                if stdout_trunc.chars().count() > 12000 {
                    let safe_head: String = stdout_trunc.chars().take(12000).collect();
                    stdout_trunc =
                        format!("{}\n...[STDOUT TRUNCATED: OUTPUT TOO LARGE]", safe_head);
                }
                if stderr_trunc.chars().count() > 1500 {
                    let total_chars = stderr_trunc.chars().count();
                    let safe_tail: String =
                        stderr_trunc.chars().skip(total_chars.saturating_sub(1500)).collect();
                    stderr_trunc = format!("[... TRUNCATED TO SAVE TOKENS ...]\n{}", safe_tail);
                }

                let mut res = format!(
                    "[CMD_OK] Command: {}\nStatus: {}\n{}\n",
                    command.trim(),
                    out.status,
                    if format!("{}", out.status).contains("exit status: 0") {
                        crate::cmd_match::cmd_hash_marker(
                            crate::cmd_match::strip_workspace_cd_prefix(command.trim()),
                        )
                    } else {
                        String::new()
                    }
                );
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

                // A shell command that redirects into a file (`>`/`>>`, including a
                // heredoc like `cat > path << 'EOF'`) mutates the filesystem exactly
                // like a native WriteFile action, but previously never emitted the
                // [WRITE_OK] marker that OutcomeVerifier/completion_engine's
                // deterministic contract checks require -- causing a correct write
                // to loop forever on rejected HALT proposals. Detect the common
                // redirect forms and synthesize the same marker on success.
                if out.status.success() {
                    if let Some(write_target) = crate::executor::detect_shell_write_target(&command)
                    {
                        res.push_str(&format!("[WRITE_OK] {}\n", write_target));
                    }
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
                let _ = memory
                    .store_memory_with_episode(
                        "WriteFile",
                        &path,
                        &content,
                        TaintLabel::MUTATED,
                        emb,
                        &self.episode,
                    )
                    .await;

                Ok(format!("[WRITE_OK] {}", path))
            }
            MotorAction::ReadFile { path } => {
                let safe_path = sanitize(&path)?;
                // A directory cannot be read as a file (EISDIR). Return a sorted
                // listing under the same [READ_OK] marker so a path-bound read of a
                // directory is satisfiable, but under a distinct header so the
                // coverage ledger (keyed on FULL FILE CONTENT:) does not treat a
                // listing as a fully-read file.
                let is_dir_listing =
                    tokio::fs::metadata(&safe_path).await.map(|m| m.is_dir()).unwrap_or(false);
                let content = if is_dir_listing {
                    let mut names: Vec<String> = Vec::new();
                    let mut rd = tokio::fs::read_dir(&safe_path).await?;
                    while let Some(entry) = rd.next_entry().await? {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                        names.push(if is_dir { format!("{}/", name) } else { name });
                    }
                    names.sort();
                    let total_entries = names.len();
                    names.truncate(200);
                    let base = path.trim_end_matches('/');
                    let mut listing = format!(
                        "ENTRIES: {} (read individual files with read_file or read_file_chunk)\n",
                        total_entries
                    );
                    for n in &names {
                        listing.push_str(&format!("{}/{}\n", base, n));
                    }
                    listing
                } else {
                    tokio::fs::read_to_string(&safe_path).await?
                };
                let snippet: String = content.chars().take(4000).collect();
                if let Ok(emb) =
                    nexus_substrate::vector_bridge::get_embedding(&self.api_key, &snippet).await
                {
                    let _ = memory
                        .store_memory_with_episode(
                            "ReadFile",
                            &path,
                            &content,
                            TaintLabel::OBSERVED,
                            Some(emb),
                            &self.episode,
                        )
                        .await;
                } else {
                    let _ = memory
                        .store_memory_with_episode(
                            "ReadFile",
                            &path,
                            &content,
                            TaintLabel::OBSERVED,
                            None,
                            &self.episode,
                        )
                        .await;
                }

                let total_lines = content.lines().count();
                // Files up to this many chars are returned whole so the coverage
                // ledger, strategist and critic never see a partial read.
                const READ_TRUNCATE_CHARS: usize = 60_000;
                let return_content = if content.chars().count() > READ_TRUNCATE_CHARS {
                    let prefix: String = content.chars().take(READ_TRUNCATE_CHARS).collect();
                    format!("{}...\n\n[CONTENT TRUNCATED TO SAVE TOKENS. TOTAL LINES: {}. USE THE `read_file_chunk` TOOL BY SPECIFYING `start_line` AND `max_lines` TO READ BEYOND THIS POINT.]", prefix, total_lines)
                } else {
                    content.clone()
                };

                let header =
                    if is_dir_listing { "DIRECTORY LISTING:" } else { "FULL FILE CONTENT:" };
                Ok(format!("[READ_OK] {}\n{}\n{}", path, header, return_content))
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
                        .store_memory_with_episode(
                            "ReadFunction",
                            &format!("{}:{}", path, function_name),
                            &source,
                            TaintLabel::OBSERVED,
                            Some(emb),
                            &self.episode,
                        )
                        .await;
                } else {
                    let _ = memory
                        .store_memory_with_episode(
                            "ReadFunction",
                            &format!("{}:{}", path, function_name),
                            &source,
                            TaintLabel::OBSERVED,
                            None,
                            &self.episode,
                        )
                        .await;
                }

                Ok(format!(
                    "[READ_FUNCTION_OK] {}:{}\nFile: {} | Function: {}\nEXACT AST SOURCE:\n{}",
                    path, function_name, path, function_name, source
                ))
            }

            MotorAction::ReadFileChunk { path, start_line, max_lines } => {
                let safe_path = sanitize(&path)?;
                let content = tokio::fs::read_to_string(&safe_path).await?;
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                // start_line is 1-based (matches the coverage ledger and the
                // model's own arithmetic). Slicing with the raw value skipped
                // the first requested line while the label still claimed it.
                // Tiny chunks make large-file audits unfinishable, so enforce
                // a floor and a cap.
                let start_idx = start_line.saturating_sub(1);
                let max_lines = max_lines.clamp(250, 400);

                if start_idx >= total_lines {
                    return Ok(format!(
                        "[READ_CHUNK_EOF] Reached end of file {}. Total lines: {}",
                        path, total_lines
                    ));
                }

                let end_line = std::cmp::min(start_idx + max_lines, total_lines);
                let chunk = lines[start_idx..end_line].join("\n");

                Ok(format!(
                    "[READ_CHUNK_OK] File: {} | Lines {}-{} of {}\nCONTENT:\n{}",
                    path,
                    start_idx + 1,
                    end_line,
                    total_lines,
                    chunk
                ))
            }
            MotorAction::SearchCode { path, query } => {
                validate_search_query(&query)?;

                let safe_path =
                    if path.trim().is_empty() { PathBuf::from("crates") } else { sanitize(&path)? };

                let p_str = safe_path.to_str().ok_or_else(|| {
                    anyhow::anyhow!("SEARCH_FAILED: search path is not valid UTF-8")
                })?;

                // Shell-free search. rg is a fast path; grep is the
                // authoritative fallback (present on every Unix host).
                //
                // --include=*.rs / --type rust are WORKSPACE-ONLY filters:
                // documented non-Rust targets (e.g.
                // SEARCH_CODE_OK:vendor/external_repo:Cargo.toml) exist but
                // are not .rs files. Applying the filter to those roots
                // makes the criterion unsatisfiable by construction.
                let is_workspace_root = p_str == "crates"
                    || p_str == "bins"
                    || p_str.starts_with("crates/")
                    || p_str.starts_with("bins/");

                let run_argv = |program: &str, args: &[&str]| -> Result<String> {
                    match crate::podman::PodmanShield::execute_argv(program, args) {
                        Ok(o) => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
                        Err(e) => {
                            let s = e.to_string();
                            if s.contains("Command failed with status: exit status: 1\n") {
                                return Ok(String::new());
                            }
                            Err(anyhow::anyhow!("{}", s))
                        }
                    }
                };

                let rg_args: Vec<&str> = if is_workspace_root {
                    vec!["--type", "rust", "-n", "-C", "2", "-i", "-e", query.as_str(), p_str]
                } else {
                    vec!["-n", "-C", "2", "-i", "-e", query.as_str(), p_str]
                };

                let mut grep_args: Vec<&str> = vec!["-rn", "-C", "2"];
                if is_workspace_root {
                    grep_args.push("--include=*.rs");
                }
                grep_args.extend_from_slice(&[
                    "--exclude-dir=.git",
                    "--exclude-dir=target",
                    "--exclude-dir=.surgical_backup",
                    "--exclude-dir=.nexus",
                    "--exclude-dir=.lazarus",
                    "--exclude-dir=node_modules",
                    "-iE",
                    "-e",
                    query.as_str(),
                    p_str,
                ]);

                // rg first. Any rg failure (binary missing, exit>=2) or
                // empty output falls through to grep. grep's failure is
                // authoritative: if rg was unavailable and grep also
                // failed, the search genuinely failed.
                let rg_out = run_argv("rg", &rg_args).unwrap_or_default();
                let stdout = if !rg_out.is_empty() {
                    rg_out
                } else {
                    // Err propagates; Ok("") from grep exit-1 is the
                    // confirmed no-match outcome.
                    match run_argv("grep", &grep_args) {
                        Ok(s) if s.is_empty() => {
                            return Ok(format!(
                                "[SEARCH_CODE_OK] Path: {} | Query: '{}'\nNo code matches found in specified path.",
                                p_str, query
                            ));
                        }
                        Ok(s) => s,
                        Err(e) => {
                            return Err(anyhow::anyhow!(
                                "SEARCH_FAILED path={} query={:?}: {}",
                                p_str,
                                query,
                                e
                            ));
                        }
                    }
                };

                let trunc = if stdout.chars().count() > 8000 {
                    let prefix: String = stdout.chars().take(8000).collect();
                    format!("{}...\n[MATCHES TRUNCATED: OUTPUT TOO LARGE]", prefix)
                } else {
                    stdout
                };

                Ok(format!(
                    "[SEARCH_CODE_OK] Path: {} | Query: '{}'\nMATCHES:\n{}",
                    p_str, query, trunc
                ))
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
                *self.action_cache.lock().await = crate::memory_cortex::ActionCache::new();
                Ok(format!(
                    "[REPLACE_LINES_OK] Replaced lines {}-{} in {}",
                    start_line, end_line, path
                ))
            }
            MotorAction::RunTests { test_target } => {
                if !crate::cortical::util::is_safe_test_target(&test_target) {
                    return Ok(format!(
                        "[TEST_REJECTED] test_target contains characters outside the allowed set: '{}'",
                        test_target
                    ));
                }
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
                *self.action_cache.lock().await = crate::memory_cortex::ActionCache::new();
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
                let _ = memory
                    .store_memory_with_episode(
                        "DiffPatch",
                        &path,
                        &patched,
                        TaintLabel::MUTATED,
                        emb,
                        &self.episode,
                    )
                    .await;

                Ok(format!("[DIFF_PATCH_OK] Applied changes to {}", path))
            }
            MotorAction::PatchFunction { path, function_name, new_body } => {
                let safe_path = sanitize(&path)?;

                // Use the path-aware structural mutation directly. This is
                // the same AST mutation path used by nexus_ast_tool itself.
                let source = tokio::fs::read_to_string(&safe_path).await?;

                let patched = nexus_ast_tool::mutation::ReplaceFunctionBody::apply_for_path(
                    &safe_path,
                    &source,
                    &function_name,
                    &new_body,
                )
                .map_err(|e| anyhow!("PatchFunction failed for '{}': {}", function_name, e))?;

                // The requested patch is already physically satisfied.
                // This is success, but it is NOT a mutation: do not rewrite
                // the file, broadcast a file shift, invalidate the action
                // cache, or record a new TaintLabel::MUTATED memory.
                if patched == source {
                    return Ok(format!(
                        "[PATCH_IDEMPOTENT_OK] {}:{}\nPatchFunction already satisfied; source unchanged.",
                        path, function_name
                    ));
                }

                // Optional one-shot fault injection (test-only, env-gated).
                // This does NOT write to disk and does NOT touch memory.
                // It computes an alternative `final_source` in memory so
                // the subsequent single write, broadcast, cache reset, and
                // memory store all reference exactly one coherent source
                // state.
                //
                // Precedence: the on-disk file, the memory record, and the
                // [PATCH_FUNCTION_OK] marker all describe `final_source`,
                // never `patched`. If injection is disabled or yields no
                // change, `final_source == patched` and behavior is
                // identical to the un-instrumented path.
                let final_source =
                    match maybe_injected_source(&safe_path, &patched, &function_name, &new_body) {
                        Some(corrupted) => corrupted,
                        None => patched,
                    };

                tokio::fs::write(&safe_path, &final_source).await?;
                _dist.broadcast_file_shift(&path, "patch_function");
                *self.action_cache.lock().await = crate::memory_cortex::ActionCache::new();

                let snippet: String = final_source.chars().take(4000).collect();
                let emb = nexus_substrate::vector_bridge::get_embedding(&self.api_key, &snippet)
                    .await
                    .ok();
                let _ = memory
                    .store_memory_with_episode(
                        "PatchFunction",
                        &path,
                        &final_source,
                        TaintLabel::MUTATED,
                        emb,
                        &self.episode,
                    )
                    .await;

                Ok(format!(
                    "[PATCH_FUNCTION_OK] {}:{}\nUpdated function '{}' in {}",
                    path, function_name, function_name, path
                ))
            }
            MotorAction::SearchMemory { query } => {
                let emb = nexus_substrate::vector_bridge::get_embedding(&self.api_key, &query)
                    .await
                    .map_err(|e| anyhow!("SEMANTIC_SEARCH_EMBEDDING_FAILED: {}", e))?;

                let results = memory
                    .semantic_search_scoped(
                        &emb,
                        12,
                        nexus_substrate::episode::RecallScope::ExplicitCrossEpisode,
                        &self.episode,
                        nexus_substrate::taint::FlowPolicy::INTERNAL_SINK,
                    )
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
                let host_terms =
                    ["uptime", "disk", "filesystem", "available", "memory", "ram", "load average"];

                let host_term_hits =
                    host_terms.iter().filter(|term| query_lower.contains(**term)).count();

                let host_observation_query = host_term_hits >= 2;

                let mut ranked_results: Vec<_> = results
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

                            let term_hits =
                                host_terms.iter().filter(|term| content.contains(**term)).count();

                            let factual_origin = matches!(
                                r.origin.to_lowercase().as_str(),
                                "readfile" | "executecommand" | "execute_command"
                            );

                            let reflection_memory =
                                r.origin.eq_ignore_ascii_case("ReflectionEngine");

                            let actual_metric_output =
                                ["filesystem", "mem:", "load average", "total used free", " up "]
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

                ranked_results.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

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
                    .store_memory_with_episode(
                        "WebScrape",
                        &url,
                        &persisted,
                        TaintLabel::UNTRUSTED_PUBLIC,
                        embedding,
                        &self.episode,
                    )
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
                *self.action_cache.lock().await = crate::memory_cortex::ActionCache::new();
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

                let models = match nexus_algorithms::Router::select("synthesize") {
                    Some(nexus_algorithms::ModelTier::Heavy) => {
                        crate::model_config::get_heavy_models()
                    }
                    Some(nexus_algorithms::ModelTier::Light) => {
                        crate::model_config::get_light_models()
                    }
                    Some(nexus_algorithms::ModelTier::Planner) => {
                        crate::model_config::get_planner_models()
                    }
                    None => crate::model_config::get_heavy_models(),
                };

                let client = reqwest::Client::new();

                match crate::epistemic_engine::EpistemicEngine::safe_api_call(
                    &client,
                    &self.api_key,
                    &models,
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
                            "[WRITE_OK] {}\nSuccessfully synthesized and wrote report to {}",
                            output_path, output_path
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

#[cfg(test)]
mod search_query_validator_tests {
    use super::validate_search_query;

    #[test]
    fn empty_query_is_err() {
        assert!(validate_search_query("").is_err());
    }

    #[test]
    fn whitespace_only_query_is_err() {
        assert!(validate_search_query("   \t  ").is_err());
    }

    #[test]
    fn newline_in_query_is_err() {
        assert!(validate_search_query("foo\nbar").is_err());
    }

    #[test]
    fn carriage_return_in_query_is_err() {
        assert!(validate_search_query("foo\rbar").is_err());
    }

    #[test]
    fn normal_query_is_ok() {
        assert!(validate_search_query("unimplemented!").is_ok());
        assert!(validate_search_query("fn main").is_ok());
    }
}
