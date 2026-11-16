use crate::motor_cortex::{MotorAction, MotorCommand};
use crate::policy::ExecutionSandbox;
use anyhow::{anyhow, Result};

/// Parse the most recent `[CONTRACT_FIRST_FORCED] search_code:<path>:<query>`
/// line from evidence. Returns `None` when no such line exists or when
/// either component is empty after trimming.
pub fn forced_search_from_evidence(evidence: &str) -> Option<(String, String)> {
    evidence.lines().rev().find_map(|line| {
        let stripped = line.trim_start().strip_prefix("[CONTRACT_FIRST_FORCED] search_code:")?;
        let (path, query) = stripped.split_once(':')?;
        let path = path.trim();
        let query = query.trim();
        if path.is_empty() || query.is_empty() {
            return None;
        }
        Some((path.to_string(), query.to_string()))
    })
}

/// Enforce that a `SearchCode` action matches the currently-forced
/// `search_code` strategy in evidence.
///
/// Skipped when:
/// * no forced `search_code` line exists in evidence,
/// * the forced target is already satisfied by a `[SEARCH_CODE_OK]`
///   block in evidence (so the cortex can move on to other work), or
/// * the action is not a `SearchCode`.
///
/// Returns `ACTION_GATE_STRATEGY_MISMATCH` on path or query divergence.
fn forced_search_paths_match(actual: &str, expected: &str) -> bool {
    let a = actual.trim();
    let e = expected.trim();
    if a == e {
        return true;
    }
    if a.split('/').any(|c| c == "..") || e.split('/').any(|c| c == "..") {
        return false;
    }
    let norm = |p: &str| -> String {
        let mut s = p.trim_start_matches("./").trim_end_matches('/').to_string();
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_s = cwd.to_string_lossy().trim_end_matches('/').to_string();
            if s == cwd_s {
                return ".".to_string();
            }
            if let Some(rest) = s.strip_prefix(&format!("{}/", cwd_s)) {
                s = rest.to_string();
            }
        }
        let s = s.trim_start_matches("./").trim_end_matches('/');
        if s.is_empty() { ".".to_string() } else { s.to_string() }
    };
    norm(a) == norm(e)
}

pub fn check_forced_search(evidence: &str, action: &MotorAction) -> Result<()> {
    let Some((expected_path, expected_query)) = forced_search_from_evidence(evidence) else {
        return Ok(());
    };
    let target = format!("{}:{}", expected_path, expected_query);
    if crate::evidence_markers::search_checked_satisfied(evidence, &target) {
        return Ok(());
    }
    if let MotorAction::SearchCode { path, query } = action {
        if !forced_search_paths_match(path, &expected_path)
            || !query.trim().eq_ignore_ascii_case(expected_query.trim())
        {
            return Err(anyhow!(
                "ACTION_GATE_STRATEGY_MISMATCH: forced strategy requires search_code:{}:{}; got search_code:{}:{}",
                expected_path,
                expected_query,
                path,
                query
            ));
        }
    }
    Ok(())
}

pub struct ActionGate {
    sandbox: ExecutionSandbox,
}

impl ActionGate {
    pub fn new() -> Self {
        Self { sandbox: ExecutionSandbox::strict_lockdown() }
    }

    pub fn with_capabilities(caps: Vec<crate::policy::Capability>) -> Self {
        Self { sandbox: ExecutionSandbox { active_capabilities: caps } }
    }

    pub fn approve(&self, command: &MotorCommand) -> Result<()> {
        match &command.action {
            MotorAction::PolyglotAstPatch { path, .. } => {
                if path.contains("..") {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Path traversal denied in AST patch."
                    ));
                }
                self.sandbox
                    .validate_intent("patch_function")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::PatchFile { .. } => {
                self.sandbox
                    .validate_intent("patch_file")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::ExecuteCommand { command: cmd_str } => {
                let cmd_lower = cmd_str.to_lowercase();
                if cmd_lower.contains("rm -rf /")
                    || cmd_lower.contains("rm -rf ~")
                    || cmd_lower.contains("mkfs")
                {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Destructive command blocked: '{}'",
                        cmd_str
                    ));
                }
                if cmd_lower.contains("cat .env")
                    || cmd_lower.contains("env |")
                    || cmd_lower.contains("printenv")
                {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Secret inspection command blocked: '{}'",
                        cmd_str
                    ));
                }
                self.sandbox
                    .validate_intent("execute_command")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::WriteFile { path, content } => {
                if path.contains("..") {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Path traversal detected: '{}'",
                        path
                    ));
                }
                if content.contains("GEMINI_API_KEY=") || content.contains("NVIDIA_API_KEY=") {
                    return Err(anyhow!("ACTION_GATE_SECURITY_VIOLATION: Plaintext API key leak blocked in WriteFile."));
                }
                self.sandbox
                    .validate_intent("write_file")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::ReadFile { path } => {
                if crate::secrets_guard::is_secret_path(path) {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: secret material read blocked: '{}'",
                        path
                    ));
                }
                self.sandbox
                    .validate_intent("read_file")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::ReadFunction { .. } => {
                self.sandbox
                    .validate_intent("read_function")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::ReadFileChunk { path, .. } => {
                if crate::secrets_guard::is_secret_path(path) {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: secret material read blocked: '{}'",
                        path
                    ));
                }
                self.sandbox
                    .validate_intent("read_file_chunk")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::SearchCode { .. } => {
                self.sandbox
                    .validate_intent("search_code")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::ReplaceLines { .. } => {
                self.sandbox
                    .validate_intent("replace_lines")
                    .map_err(|e| anyhow::anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::RunTests { .. } => {
                self.sandbox
                    .validate_intent("run_tests")
                    .map_err(|e| anyhow::anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::AtomicPatch { patches } => {
                for patch in patches {
                    let raw_path = std::path::Path::new(&patch.path);
                    if raw_path.components().any(|c| c.as_os_str() == "..") {
                        return Err(anyhow::anyhow!("ACTION_GATE_SECURITY_VIOLATION: Path traversal denied via .. in AtomicPatch"));
                    }
                }
                self.sandbox
                    .validate_intent("write_file")
                    .map_err(|e| anyhow::anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::DiffPatch { path, .. } => {
                if path.contains("..") {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Path traversal detected: '{}'",
                        path
                    ));
                }
                self.sandbox
                    .validate_intent("diff_patch")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::PatchFunction { path, .. } => {
                if path.contains("..") {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Path traversal detected: '{}'",
                        path
                    ));
                }
                self.sandbox
                    .validate_intent("patch_function")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::SearchMemory { .. } => {
                self.sandbox
                    .validate_intent("search_memory")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::ScrapeUrl { url, prompt } => {
                let lower = url.to_lowercase();
                if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Scrape URL must use http:// or https://: '{}'",
                        url
                    ));
                }

                let after_scheme = if lower.starts_with("https://") { &url[8..] } else { &url[7..] };
                let host_only = after_scheme.split(['/', '?', '#']).next().unwrap_or(after_scheme);

                if host_only.trim().is_empty() {
                    return Err(anyhow!("ACTION_GATE_SECURITY_VIOLATION: Scrape URL has no host."));
                }

                if crate::secrets_guard::host_is_private(host_only) {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Scrape URL targets a private host: '{}'. \
                         NOTE: string-only check; does NOT resolve DNS or follow redirects. \
                         A public host that 302s to a private address is still allowed through.",
                        host_only
                    ));
                }

                if prompt.trim().is_empty() {
                    return Err(anyhow!(
                        "ACTION_GATE_SECURITY_VIOLATION: Scrape extraction prompt cannot be empty."
                    ));
                }

                self.sandbox
                    .validate_intent("scrape_url")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::SynthesizeReport { .. } => {
                self.sandbox
                    .validate_intent("write_file")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
            MotorAction::Halt { .. } => {
                self.sandbox
                    .validate_intent("halt")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
        }
        Ok(())
    }

    /// Approve a motor command, additionally enforcing that a forced
    /// `search_code` strategy is followed exactly.
    ///
    /// Delegates to `approve` for every check other than the forced-search
    /// match, so non-search actions and non-forced searches behave
    /// identically to the plain `approve` path.
    ///
    /// Rejection travels through the same `Result<()>` channel as every
    /// other `approve` failure. Both current call sites
    /// (`sovereign_core::execute_action` and
    /// `CorticalLoop::execute_actions`) handle `Err` by logging,
    /// adding an observation node, and continuing — no new termination
    /// behavior is introduced.
    pub fn approve_with_forced_search(&self, command: &MotorCommand, evidence: &str) -> Result<()> {
        check_forced_search(evidence, &command.action)?;
        self.approve(command)
    }
}

impl Default for ActionGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod forced_search_gate_tests {
    use super::{check_forced_search, forced_search_from_evidence};
    use crate::motor_cortex::MotorAction;

    fn search(path: &str, query: &str) -> MotorAction {
        MotorAction::SearchCode { path: path.to_string(), query: query.to_string() }
    }

    #[test]
    fn matching_path_and_query_are_accepted() {
        let evidence = "[CONTRACT_FIRST_FORCED] search_code:Cargo.toml:workspace\n";
        let action = search("Cargo.toml", "workspace");
        assert!(check_forced_search(evidence, &action).is_ok());
    }

    #[test]
    fn path_mismatch_is_rejected() {
        let evidence = "[CONTRACT_FIRST_FORCED] search_code:Cargo.toml:workspace\n";
        let action = search("vendor/external_repo", "workspace");
        let err = check_forced_search(evidence, &action).unwrap_err();
        assert!(err.to_string().contains("ACTION_GATE_STRATEGY_MISMATCH"));
    }

    #[test]
    fn query_mismatch_is_rejected() {
        let evidence = "[CONTRACT_FIRST_FORCED] search_code:Cargo.toml:workspace\n";
        let action = search("Cargo.toml", "HAND.toml");
        let err = check_forced_search(evidence, &action).unwrap_err();
        assert!(err.to_string().contains("ACTION_GATE_STRATEGY_MISMATCH"));
    }

    #[test]
    fn satisfied_target_does_not_block_a_different_search() {
        let evidence = concat!(
            "[CONTRACT_FIRST_FORCED] search_code:Cargo.toml:workspace\n",
            "[SEARCH_CODE_OK] Path: /root/AthenaFusionX/Cargo.toml | Query: 'workspace'\nMATCHES:\n1:[workspace]\n",
        );
        let action = search("src", "todo!");
        assert!(check_forced_search(evidence, &action).is_ok());
    }

    #[test]
    fn no_forced_line_leaves_action_unchanged() {
        let action = search("vendor/external_repo", "anything");
        assert!(check_forced_search("ordinary evidence\n", &action).is_ok());
    }

    #[test]
    fn non_search_action_with_forced_line_is_unaffected() {
        let evidence = "[CONTRACT_FIRST_FORCED] search_code:Cargo.toml:workspace\n";
        let action = MotorAction::ReadFile { path: "Cargo.toml".to_string() };
        assert!(check_forced_search(evidence, &action).is_ok());
    }

    #[test]
    fn extraction_reads_most_recent_forced_line() {
        let evidence = concat!(
            "[CONTRACT_FIRST_FORCED] search_code:old/path:oldterm\n",
            "some other evidence\n",
            "[CONTRACT_FIRST_FORCED] search_code:Cargo.toml:workspace\n",
            "more evidence\n",
        );
        assert_eq!(
            forced_search_from_evidence(evidence),
            Some(("Cargo.toml".to_string(), "workspace".to_string()))
        );
    }

    #[test]
    fn extraction_handles_dot_root_form() {
        let evidence = "[CONTRACT_FIRST_FORCED] search_code:.:todo!\n";
        assert_eq!(
            forced_search_from_evidence(evidence),
            Some((".".to_string(), "todo!".to_string()))
        );
    }

    #[test]
    fn extraction_returns_none_without_forced_line() {
        assert_eq!(forced_search_from_evidence("ordinary evidence\n"), None);
    }

    #[test]
    fn abs_cwd_path_and_trailing_slash_are_accepted() {
        let cwd = std::env::current_dir().unwrap();
        let abs = cwd.join("Cargo.toml").to_string_lossy().to_string();
        let evidence = "[CONTRACT_FIRST_FORCED] search_code:Cargo.toml:workspace\n";
        assert!(check_forced_search(evidence, &search(&abs, "workspace")).is_ok());

        let slash_ev = "[CONTRACT_FIRST_FORCED] search_code:bins/:main\n";
        assert!(check_forced_search(slash_ev, &search("bins", "main")).is_ok());
    }
}
