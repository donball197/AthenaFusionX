//! Policy Engine – Phase 8B v2
//! Capability-based authorization.

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    FilesystemRead,
    FilesystemWrite,
    Network,
    Shell,
    Patch,
    Delete,
    MemoryWrite,
    SpawnSubagent,
}

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    allowed: HashSet<Capability>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert(Capability::FilesystemRead);
        allowed.insert(Capability::FilesystemWrite);
        allowed.insert(Capability::Shell);
        allowed.insert(Capability::Patch);
        allowed.insert(Capability::MemoryWrite);
        // Delete and unrestricted Network denied by default
        Self { allowed }
    }
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allows(&self, cap: Capability) -> bool {
        self.allowed.contains(&cap)
    }

    pub fn grant(&mut self, cap: Capability) {
        self.allowed.insert(cap);
    }

    pub fn revoke(&mut self, cap: Capability) {
        self.allowed.remove(&cap);
    }

    /// Map a raw action string to required capabilities (best-effort).
    pub fn required_capabilities(action: &str) -> Vec<Capability> {
        let lower = action.to_lowercase();
        let mut caps = Vec::new();
        if lower.contains("write_file") || lower.contains("diff_patch") || lower.contains("patch_function") {
            caps.push(Capability::FilesystemWrite);
            caps.push(Capability::Patch);
        }
        if lower.contains("read_file") || lower.contains("search_code") || lower.contains("read_file_chunk") {
            caps.push(Capability::FilesystemRead);
        }
        if lower.contains("execute_command") || lower.starts_with("rg ") || lower.starts_with("find ") {
            caps.push(Capability::Shell);
        }
        if lower.contains("http") || lower.contains("curl") || lower.contains("wget") {
            caps.push(Capability::Network);
        }
        if lower.contains("rm ") || lower.contains("delete") {
            caps.push(Capability::Delete);
        }
        if lower.contains("spawn") || lower.contains("delegate") {
            caps.push(Capability::SpawnSubagent);
        }
        if caps.is_empty() {
            caps.push(Capability::Shell); // conservative default
        }
        caps
    }

    pub fn check_action(&self, action: &str) -> bool {
        Self::required_capabilities(action)
            .into_iter()
            .all(|c| self.allows(c))
    }
}
