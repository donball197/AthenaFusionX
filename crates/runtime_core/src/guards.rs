//! ActionGuard pipeline — additive module.

use crate::agent_protocol::AgentAction;
use crate::cognition::epistemic_session::EpistemicSession;

#[derive(Debug, Clone)]
pub struct GuardVeto {
    pub guard_name: String,
    pub reason: String,
    pub severity: VetoSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VetoSeverity { Pivot, Warn }

pub trait ActionGuard: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, action: &AgentAction, session: &EpistemicSession) -> Result<(), GuardVeto>;
}

pub struct AntiLoopGuard { pub max_recon: u32, pub max_recon_post_mutation: u32 }
impl Default for AntiLoopGuard { fn default() -> Self { Self { max_recon: 2, max_recon_post_mutation: 4 } } }

impl ActionGuard for AntiLoopGuard {
    fn name(&self) -> &'static str { "AntiLoop" }
    fn validate(&self, action: &AgentAction, session: &EpistemicSession) -> Result<(), GuardVeto> {
        let (action_key, is_recon) = extract_action_key(action);
        let mutation_done = session.local_evidence.contains("[WRITE_OK]")
            || session.local_evidence.contains("[DIFF_PATCH_OK]")
            || session.local_evidence.contains("[PATCH_FUNCTION_OK]")
            || session.local_evidence.contains("[REPLACE_LINES_OK]");
        let limit: u32 = if is_recon && mutation_done { self.max_recon_post_mutation } else { self.max_recon };
        let attempt_count = session.action_attempt_count(&action_key).unwrap_or(0);
        if attempt_count >= limit {
            return Err(GuardVeto {
                guard_name: self.name().to_string(),
                reason: format!("Action [{}] attempted {} times (limit {}). FORCING STRATEGY PIVOT.", action_key, attempt_count, limit),
                severity: VetoSeverity::Pivot,
            });
        }
        Ok(())
    }
}

pub struct SecurityGuard;
impl ActionGuard for SecurityGuard {
    fn name(&self) -> &'static str { "Security" }
    fn validate(&self, action: &AgentAction, _session: &EpistemicSession) -> Result<(), GuardVeto> {
        match action {
            AgentAction::ExecuteCommand { command, .. } => {
                let lower = command.to_lowercase();
                let dangerous = ["rm -rf /", "rm -rf ~", "mkfs", "chmod 777 /", "/etc/shadow", "mv /etc/", ":(){ :|:& };:", "dd if=/dev/zero", "fork bomb", "sudo su", "curl | bash", "wget | bash"];
                for pat in &dangerous {
                    if lower.contains(pat) {
                        return Err(GuardVeto { guard_name: self.name().to_string(), reason: format!("Dangerous pattern '{}' detected.", pat), severity: VetoSeverity::Pivot });
                    }
                }
            }
            AgentAction::ScrapeUrl { url, .. } => {
                let lower = url.to_lowercase();
                if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                    return Err(GuardVeto {
                        guard_name: self.name().to_string(),
                        reason: format!(
                            "Scrape URL must use http:// or https://: '{}'",
                            url
                        ),
                        severity: VetoSeverity::Pivot,
                    });
                }

                let after_scheme = if lower.starts_with("https://") {
                    &url[8..]
                } else {
                    &url[7..]
                };

                if after_scheme.trim().is_empty() {
                    return Err(GuardVeto {
                        guard_name: self.name().to_string(),
                        reason: "Scrape URL has no host.".to_string(),
                        severity: VetoSeverity::Pivot,
                    });
                }
            }
            AgentAction::WriteFile { path, .. } | AgentAction::DiffPatch { path, .. } | AgentAction::PatchFunction { path, .. } => {
                if path.contains("..") {
                    return Err(GuardVeto { guard_name: self.name().to_string(), reason: format!("Path traversal detected: '{}'", path), severity: VetoSeverity::Pivot });
                }
                if let AgentAction::WriteFile { content, .. } = action {
                    if content.contains("GEMINI_API_KEY=") || content.contains("NVIDIA_API_KEY=") {
                        return Err(GuardVeto { guard_name: self.name().to_string(), reason: "Plaintext API key leak blocked.".to_string(), severity: VetoSeverity::Pivot });
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn extract_action_key(action: &AgentAction) -> (String, bool) {
    match action {
        AgentAction::ReadFile { path, .. } => (format!("read_file:{}", path), true),
        AgentAction::ReadFunction { path, function_name, .. } => (format!("read_function:{}:{}", path, function_name), true),
        AgentAction::ReadFileChunk { path, start_line, .. } => (format!("read_file_chunk:{}:{}", path, start_line), true),
        AgentAction::SearchCode { path, query, .. } => (format!("search_code:{}:{}", path, query), true),
        AgentAction::SearchMemory { query, .. } => (format!("search_memory:{}", query), true),
        AgentAction::ScrapeUrl { url, .. } => (format!("scrape_url:{}", url), true),
        AgentAction::ExecuteCommand { command, .. } => (format!("execute_command:{}", command), false),
        AgentAction::WriteFile { path, .. } => (format!("write_file:{}", path), false),
        AgentAction::DiffPatch { path, .. } => (format!("diff_patch:{}", path), false),
        AgentAction::PatchFunction { path, function_name, .. } => (format!("patch_function:{}:{}", path, function_name), false),
        AgentAction::ReplaceLines { path, start_line, end_line, .. } => (format!("replace_lines:{}:{}:{}", path, start_line, end_line), false),
        _ => (format!("{:?}", action), false),
    }
}

pub struct GuardPipeline { guards: Vec<Box<dyn ActionGuard>> }
impl GuardPipeline {
    pub fn new() -> Self { Self { guards: vec![Box::new(SecurityGuard), Box::new(AntiLoopGuard::default())] } }
    pub fn validate(&self, action: &AgentAction, session: &EpistemicSession) -> Result<(), GuardVeto> {
        for guard in &self.guards {
            guard.validate(action, session)?;
        }
        Ok(())
    }
}
impl Default for GuardPipeline { fn default() -> Self { Self::new() } }
