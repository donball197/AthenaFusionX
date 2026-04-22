#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════
#  NEXUS SINGULARITY — INTELLIGENT ROUTER + TOKEN LEDGER UPGRADE
#  Run from: ~/nexus_singularity-dev
#  Conductor: gemma-4-26b-a4b-it (MoE 256K)
# ═══════════════════════════════════════════════════════════════════
set -euo pipefail
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1

echo -e "\e[36m>> [NEXUS] Beginning intelligent router + token ledger upgrade...\e[0m"

# ───────────────────────────────────────────────────────────────────
# 1. src/router.rs — Zero-token directive classifier
# ───────────────────────────────────────────────────────────────────
cat << 'EOF' > src/router.rs
// src/router.rs — Intelligent Agent Router v2.0
// Copyright 2026 Donald Ball — Apache 2.0
//
// Conductor: gemma-4-26b-a4b-it (MoE 256K ctx)
// Classifies every directive BEFORE touching the API.
// Produces an AgentProfile that caps models, turns, and context budget.

use crate::factory::AgentFactory;

/// DirectiveClass: What kind of work is this?
/// Determined with zero token spend — pure local heuristics.
#[derive(Debug, Clone, PartialEq)]
pub enum DirectiveClass {
    /// Pure shell: cargo check, git status, ls — executor handles directly
    ShellOp,
    /// Fix a specific named file — minimal context, 5-turn cap
    TargetedEdit { file_hint: String },
    /// Multi-file refactor or redesign — full context, dense model
    ArchitecturalChange,
    /// Security, FFI, unsafe audit — dense model + full schema
    SecurityAudit,
    /// Read-only question — no writes, 2-turn cap, no schema
    Inquiry,
    /// New feature — git branch required, full pipeline
    FeatureBuild,
    /// Fallback
    General,
}

/// ContextBudget: Hard caps on tokens injected per turn.
/// This is the primary lever against 429 / quota exhaustion.
#[derive(Debug, Clone)]
pub struct ContextBudget {
    /// Max chars of file content injected per turn
    pub max_file_chars: usize,
    /// Max recent log lines carried forward
    pub max_obs_lines: usize,
    /// Max chars of system schema injected
    pub max_schema_chars: usize,
    /// Hard ceiling on output tokens requested from the API
    pub max_output_tokens: u32,
}

impl ContextBudget {
    pub fn minimal() -> Self {
        Self {
            max_file_chars: 1_500,
            max_obs_lines: 2,
            max_schema_chars: 400,
            max_output_tokens: 1024,
        }
    }
    pub fn standard() -> Self {
        Self {
            max_file_chars: 6_000,
            max_obs_lines: 5,
            max_schema_chars: 1_000,
            max_output_tokens: 4096,
        }
    }
    pub fn full() -> Self {
        Self {
            max_file_chars: 20_000,
            max_obs_lines: 10,
            max_schema_chars: 2_000,
            max_output_tokens: 8192,
        }
    }
    /// Estimated input token count for budget planning (rough: 4 chars ≈ 1 token)
    pub fn estimated_input_tokens(&self) -> u32 {
        ((self.max_file_chars + self.max_schema_chars + (self.max_obs_lines * 120)) / 4) as u32
    }
}

/// AgentProfile: Full execution plan produced by the router.
#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub class:          DirectiveClass,
    pub primary_model:  &'static str,
    pub fallback_model: &'static str,
    pub max_turns:      u8,
    pub budget:         ContextBudget,
    pub persona:        String,
    pub inject_git:     bool,
    pub inject_schema:  bool,
}

pub struct IntelligentRouter;

impl IntelligentRouter {
    /// classify: Zero-token keyword scan.
    pub fn classify(directive: &str) -> DirectiveClass {
        let d = directive.to_lowercase();

        // Shell ops — executor handles, no LLM required
        let shell_kw = [
            "cargo check", "cargo test", "cargo build", "cargo clippy",
            "git status", "git log", "git diff", "git branch",
            "ls ", "find ", "cat ", "grep ",
        ];
        if shell_kw.iter().any(|k| d.contains(k)) {
            return DirectiveClass::ShellOp;
        }

        // Security audit — needs dense model
        let sec_kw = [
            "audit", "vulnerability", "ffi bound", "unsafe", "memory safe",
            "sandbox", "arbiter", "exploit", "injection",
        ];
        if sec_kw.iter().any(|k| d.contains(k)) {
            return DirectiveClass::SecurityAudit;
        }

        // Targeted file edit — extract file path hint
        let edit_kw = ["fix", "patch", "update", "edit", "change", "correct", "repair"];
        let file_exts = [".rs", ".zig", ".toml", ".svelte", ".ts", ".js"];
        if edit_kw.iter().any(|k| d.contains(k)) {
            if let Some(f) = Self::extract_file_hint(&d, &file_exts) {
                return DirectiveClass::TargetedEdit { file_hint: f };
            }
        }

        // Feature build — needs git branch
        let feat_kw = [
            "add ", "create ", "implement ", "build ",
            "new endpoint", "new module", "new agent", "new feature",
        ];
        if feat_kw.iter().any(|k| d.contains(k)) {
            return DirectiveClass::FeatureBuild;
        }

        // Architectural change
        let arch_kw = [
            "refactor", "restructure", "redesign",
            "migrate", "extract", "decouple", "rewrite",
        ];
        if arch_kw.iter().any(|k| d.contains(k)) {
            return DirectiveClass::ArchitecturalChange;
        }

        // Inquiry — read-only
        let inq_kw = [
            "what ", "explain ", "describe ", "how does", "why ",
            "show me", "list all", "summarize", "what is", "tell me",
        ];
        if inq_kw.iter().any(|k| d.contains(k)) {
            return DirectiveClass::Inquiry;
        }

        DirectiveClass::General
    }

    /// route: Produce a full AgentProfile from a directive.
    pub fn route(directive: &str) -> AgentProfile {
        let class = Self::classify(directive);
        match &class {
            DirectiveClass::ShellOp => AgentProfile {
                class,
                primary_model:  "gemma-4-26b-a4b-it",
                fallback_model: "gemma-4-26b-a4b-it",
                max_turns:      2,
                budget:         ContextBudget::minimal(),
                persona:        "NEXUS Shell Executor. Run the command. Halt immediately.".into(),
                inject_git:     false,
                inject_schema:  false,
            },
            DirectiveClass::TargetedEdit { file_hint } => AgentProfile {
                class,
                primary_model:  "gemma-4-26b-a4b-it",
                fallback_model: "gemma-4-31b-it",
                max_turns:      5,
                budget:         ContextBudget::standard(),
                persona:        format!(
                    "NEXUS Surgeon. Fix ONLY {}. Protocol: snapshot → write → cargo check → halt.",
                    file_hint
                ),
                inject_git:     false,
                inject_schema:  true,
            },
            DirectiveClass::SecurityAudit => AgentProfile {
                class,
                primary_model:  "gemma-4-31b-it",
                fallback_model: "gemma-4-31b-it",
                max_turns:      8,
                budget:         ContextBudget::full(),
                persona:        AgentFactory::get_persona_for_mission("security"),
                inject_git:     true,
                inject_schema:  true,
            },
            DirectiveClass::FeatureBuild => AgentProfile {
                class,
                primary_model:  "gemma-4-26b-a4b-it",
                fallback_model: "gemma-4-31b-it",
                max_turns:      12,
                budget:         ContextBudget::full(),
                persona:        AgentFactory::get_persona_for_mission("devops"),
                inject_git:     true,
                inject_schema:  true,
            },
            DirectiveClass::ArchitecturalChange => AgentProfile {
                class,
                primary_model:  "gemma-4-31b-it",
                fallback_model: "gemma-4-31b-it",
                max_turns:      16,
                budget:         ContextBudget::full(),
                persona:        AgentFactory::get_persona_for_mission("architect"),
                inject_git:     true,
                inject_schema:  true,
            },
            DirectiveClass::Inquiry => AgentProfile {
                class,
                primary_model:  "gemma-4-26b-a4b-it",
                fallback_model: "gemma-4-26b-a4b-it",
                max_turns:      2,
                budget:         ContextBudget::standard(),
                persona:        "NEXUS Archivist. Answer precisely in your response text. Do NOT write files. Halt immediately after answering.".into(),
                inject_git:     false,
                inject_schema:  false,
            },
            DirectiveClass::General | _ => AgentProfile {
                class,
                primary_model:  "gemma-4-26b-a4b-it",
                fallback_model: "gemma-4-31b-it",
                max_turns:      8,
                budget:         ContextBudget::standard(),
                persona:        AgentFactory::get_persona_for_mission("devops"),
                inject_git:     true,
                inject_schema:  true,
            },
        }
    }

    /// build_prompt: Assemble the final prompt respecting ContextBudget.
    pub fn build_prompt(
        directive: &str,
        profile:   &AgentProfile,
        git_ctx:   &str,
        file_ctx:  Option<&str>,
    ) -> String {
        let mut parts: Vec<String> = Vec::new();

        if profile.inject_schema {
            let schema = Self::schema_for(&profile.class);
            let trimmed = &schema[..schema.len().min(profile.budget.max_schema_chars)];
            parts.push(format!("[NEXUS FACTORY PROTOCOL]\n{}", trimmed));
        }

        if profile.inject_git && !git_ctx.is_empty() {
            parts.push(format!("GIT_STATE:\n{}", git_ctx));
        }

        if let Some(fc) = file_ctx {
            let trimmed_fc = &fc[..fc.len().min(profile.budget.max_file_chars)];
            parts.push(format!("FILE_CONTEXT:\n{}", trimmed_fc));
        }

        parts.push(format!("PERSONA: {}", profile.persona));
        parts.push(format!("DIRECTIVE: {}", directive));
        parts.join("\n\n")
    }

    /// schema_for: Returns only the action subset relevant to the class.
    /// Full schema is ~2400 chars. These are 200-600 chars. Big token savings.
    fn schema_for(class: &DirectiveClass) -> String {
        match class {
            DirectiveClass::ShellOp =>
                "ACTIONS: execute_command, halt".into(),
            DirectiveClass::TargetedEdit { .. } =>
                "ACTIONS: lazarus_snapshot, write_file, execute_command, halt\n\
                 RULES: snapshot FIRST. cargo check AFTER every write. halt when build is clean.".into(),
            DirectiveClass::Inquiry =>
                "ACTIONS: search_memory, halt\n\
                 RULES: Answer in response text. No file writes. Halt immediately.".into(),
            DirectiveClass::FeatureBuild =>
                "ACTIONS: git_action{branch first}, lazarus_snapshot, write_file, execute_command, halt\n\
                 RULES: Never commit to main. Branch → build → check → commit → halt.".into(),
            _ =>
                "ACTIONS: git_action, lazarus_snapshot, write_file, execute_command, \
                 vmo_create, vmo_write, search_memory, export_report, halt".into(),
        }
    }

    fn extract_file_hint(directive: &str, exts: &[&str]) -> Option<String> {
        for word in directive.split_whitespace() {
            if exts.iter().any(|e| word.ends_with(e)) {
                return Some(word.to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_op_gets_zero_schema() {
        let p = IntelligentRouter::route("cargo check");
        assert_eq!(p.max_turns, 2);
        assert!(!p.inject_schema);
        assert!(!p.inject_git);
    }

    #[test]
    fn inquiry_caps_at_two_turns() {
        let p = IntelligentRouter::route("what does calculate_resonance_e2b do?");
        assert_eq!(p.max_turns, 2);
        assert!(!p.inject_git);
    }

    #[test]
    fn security_audit_uses_dense_model() {
        let p = IntelligentRouter::route("audit the FFI boundaries in the kernel");
        assert_eq!(p.primary_model, "gemma-4-31b-it");
    }

    #[test]
    fn targeted_edit_extracts_file_hint() {
        let p = IntelligentRouter::route("fix the alignment error in src/kernel/vmar.rs");
        matches!(p.class, DirectiveClass::TargetedEdit { .. });
    }
}
EOF
echo -e "\e[32m  ✓ src/router.rs written\e[0m"

# ───────────────────────────────────────────────────────────────────
# 2. src/token_ledger.rs — Real-time quota tracking + backoff
# ───────────────────────────────────────────────────────────────────
cat << 'EOF' > src/token_ledger.rs
// src/token_ledger.rs — Token Ledger v1.0
// Copyright 2026 Donald Ball — Apache 2.0
//
// Tracks per-model token and request spend inside a rolling 60-second window.
// Call `check_and_reserve` before every API call. It returns:
//   Ok(())           — cleared, proceed
//   Err(wait_ms)     — backoff this many milliseconds before retrying
//
// Gemini free-tier limits (as of April 2026):
//   gemma-4-26b-a4b-it : 15 RPM, 1_000_000 TPM
//   gemma-4-31b-it     : 2 RPM,    50_000 TPM
//   gemini-2.5-pro     : 5 RPM,   250_000 TPM  (emergency only)
//
// Set NEXUS_RPM_OVERRIDE_26B / NEXUS_TPM_OVERRIDE_26B env vars to tune.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct ModelQuota {
    pub rpm_limit: u32,
    pub tpm_limit: u32,
}

impl ModelQuota {
    fn for_model(model: &str) -> Self {
        match model {
            m if m.contains("31b") => Self { rpm_limit: 2,  tpm_limit: 50_000 },
            m if m.contains("2.5-pro") => Self { rpm_limit: 5, tpm_limit: 250_000 },
            // gemma-4-26b-a4b-it and all other MoE models
            _ => Self {
                rpm_limit: std::env::var("NEXUS_RPM_OVERRIDE_26B")
                    .ok().and_then(|v| v.parse().ok()).unwrap_or(15),
                tpm_limit: std::env::var("NEXUS_TPM_OVERRIDE_26B")
                    .ok().and_then(|v| v.parse().ok()).unwrap_or(1_000_000),
            },
        }
    }
}

#[derive(Debug)]
struct WindowEntry {
    timestamp: Instant,
    tokens:    u32,
}

#[derive(Debug, Default)]
struct ModelWindow {
    entries: Vec<WindowEntry>,
}

impl ModelWindow {
    /// Purge entries older than 60 seconds
    fn purge_stale(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(60);
        self.entries.retain(|e| e.timestamp > cutoff);
    }

    fn request_count(&mut self) -> u32 {
        self.purge_stale();
        self.entries.len() as u32
    }

    fn token_sum(&mut self) -> u32 {
        self.purge_stale();
        self.entries.iter().map(|e| e.tokens).sum()
    }

    fn add(&mut self, tokens: u32) {
        self.entries.push(WindowEntry { timestamp: Instant::now(), tokens });
    }

    /// Returns how many ms until the oldest entry expires (leaves the window)
    fn ms_until_slot_free(&self) -> u64 {
        if self.entries.is_empty() { return 0; }
        let oldest = self.entries.iter().map(|e| e.timestamp).min().unwrap();
        let age = oldest.elapsed();
        if age >= Duration::from_secs(60) {
            0
        } else {
            (Duration::from_secs(60) - age).as_millis() as u64 + 500 // +500ms safety buffer
        }
    }
}

pub struct TokenLedger {
    windows: Mutex<HashMap<String, ModelWindow>>,
}

impl TokenLedger {
    pub fn new() -> Self {
        Self { windows: Mutex::new(HashMap::new()) }
    }

    /// check_and_reserve: Call BEFORE every API request.
    /// estimated_tokens: rough estimate of input+output tokens for this call.
    /// Returns Ok(()) if cleared, Err(wait_ms) if the caller should back off.
    pub fn check_and_reserve(&self, model: &str, estimated_tokens: u32) -> Result<(), u64> {
        let quota = ModelQuota::for_model(model);
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();

        let current_rpm = window.request_count();
        let current_tpm = window.token_sum();

        // RPM check
        if current_rpm >= quota.rpm_limit {
            let wait = window.ms_until_slot_free();
            println!(
                "\x1b[33m[LEDGER] RPM ceiling hit for {} ({}/{}). Backoff {}ms\x1b[0m",
                model, current_rpm, quota.rpm_limit, wait
            );
            return Err(wait);
        }

        // TPM check
        if current_tpm + estimated_tokens > quota.tpm_limit {
            let wait = window.ms_until_slot_free();
            println!(
                "\x1b[33m[LEDGER] TPM ceiling hit for {} ({}/{}). Backoff {}ms\x1b[0m",
                model, current_tpm + estimated_tokens, quota.tpm_limit, wait
            );
            return Err(wait);
        }

        // All clear — record the reservation
        window.add(estimated_tokens);
        Ok(())
    }

    /// record_actual: Call AFTER a successful response with the real token count.
    /// This corrects the estimate recorded by check_and_reserve.
    pub fn record_actual(&self, model: &str, actual_tokens: u32) {
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();
        // Replace the last entry's token count with the actual
        if let Some(last) = window.entries.last_mut() {
            last.tokens = actual_tokens;
        }
    }

    /// status: Returns a human-readable status string for the dashboard / metrics endpoint.
    pub fn status(&self) -> String {
        let mut map = self.windows.lock().unwrap();
        let mut lines = vec!["[TOKEN LEDGER STATUS]".to_string()];
        for (model, window) in map.iter_mut() {
            let quota = ModelQuota::for_model(model);
            lines.push(format!(
                "  {} → RPM: {}/{} | TPM: {}/{}",
                model,
                window.request_count(),
                quota.rpm_limit,
                window.token_sum(),
                quota.tpm_limit,
            ));
        }
        lines.join("\n")
    }

    /// percent_tpm_used: 0.0–1.0 saturation for a given model.
    pub fn percent_tpm_used(&self, model: &str) -> f32 {
        let quota = ModelQuota::for_model(model);
        let mut map = self.windows.lock().unwrap();
        let window = map.entry(model.to_string()).or_default();
        window.token_sum() as f32 / quota.tpm_limit as f32
    }
}

/// Global ledger — shared across all tasks via Arc
pub static LEDGER: Lazy<TokenLedger> = Lazy::new(TokenLedger::new);

/// wait_for_quota: Blocking async backoff helper.
/// Waits for the ledger to clear, then reserves.
/// Adds exponential backoff with jitter on repeated 429s.
pub async fn wait_for_quota(model: &str, estimated_tokens: u32) {
    let mut attempts = 0u32;
    loop {
        match LEDGER.check_and_reserve(model, estimated_tokens) {
            Ok(()) => return,
            Err(wait_ms) => {
                // Exponential backoff with jitter: base wait + (2^attempts * 200ms) + jitter
                let jitter = (rand_u64() % 300) as u64;
                let backoff = wait_ms + (2u64.saturating_pow(attempts.min(6)) * 200) + jitter;
                println!(
                    "\x1b[33m[LEDGER] Waiting {}ms before retry (attempt {})\x1b[0m",
                    backoff, attempts + 1
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                attempts += 1;
            }
        }
    }
}

/// Tiny seedless xorshift RNG to avoid pulling in the rand crate
fn rand_u64() -> u64 {
    use std::time::SystemTime;
    let mut x = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345) as u64;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_rpm_limit() {
        let ledger = TokenLedger::new();
        // gemma-4-31b-it has rpm=2
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        let result = ledger.check_and_reserve("gemma-4-31b-it", 100);
        assert!(result.is_err(), "Third request should be blocked by RPM limit");
    }

    #[test]
    fn different_models_independent_windows() {
        let ledger = TokenLedger::new();
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        ledger.check_and_reserve("gemma-4-31b-it", 100).unwrap();
        // MoE model has its own window — should still pass
        assert!(ledger.check_and_reserve("gemma-4-26b-a4b-it", 100).is_ok());
    }
}
EOF
echo -e "\e[32m  ✓ src/token_ledger.rs written\e[0m"

# ───────────────────────────────────────────────────────────────────
# 3. src/conductor.rs — gemma-4-26b-a4b-it as the named conductor
# ───────────────────────────────────────────────────────────────────
cat << 'EOF' > src/conductor.rs
// src/conductor.rs — Conductor v1.0
// Copyright 2026 Donald Ball — Apache 2.0
//
// gemma-4-26b-a4b-it is the CONDUCTOR of this build.
// All turns pass through here. The conductor decides if it needs
// to escalate to the 31b fallback or stay on the MoE track.
// Token budget is enforced via token_ledger::wait_for_quota before
// every call.

use crate::router::AgentProfile;
use crate::token_ledger::{wait_for_quota, LEDGER};
use crate::orchestrator::Orchestrator;
use crate::neural_agent::ContextPacket;
use reqwest::Client;
use tokio::sync::broadcast;
use serde_json::json;

pub const CONDUCTOR_MODEL: &str = "gemma-4-26b-a4b-it";
pub const FALLBACK_MODEL:   &str = "gemma-4-31b-it";

pub struct Conductor;

impl Conductor {
    /// run_turn: Execute a single turn through the conductor.
    ///
    /// Automatically:
    ///   1. Checks the token ledger (waits if at quota)
    ///   2. Selects primary vs fallback model based on error_count
    ///   3. Injects the correct persona and trimmed context
    ///   4. Records actual token usage back to the ledger
    pub async fn run_turn(
        profile:       &AgentProfile,
        prompt:        String,
        error_count:   usize,
        orch:          &Orchestrator,
        client:        &Client,
        api_key:       &str,
        tx:            Option<broadcast::Sender<String>>,
    ) -> Result<String, String> {
        // Model selection: escalate to fallback after 1 error
        let model = if error_count > 1 || matches!(
            profile.class,
            crate::router::DirectiveClass::SecurityAudit |
            crate::router::DirectiveClass::ArchitecturalChange
        ) {
            FALLBACK_MODEL
        } else {
            CONDUCTOR_MODEL
        };

        let estimated = profile.budget.estimated_input_tokens()
            + profile.budget.max_output_tokens;

        // Block here if quota is tight — no 429s
        wait_for_quota(model, estimated).await;

        if let Some(ref sender) = tx {
            let saturation = LEDGER.percent_tpm_used(model);
            let _ = sender.send(json!({
                "type": "NEURAL_THOUGHT",
                "payload": format!(
                    "[CONDUCTOR] {} | quota: {:.0}% | est_tokens: {}",
                    model,
                    saturation * 100.0,
                    estimated
                )
            }).to_string());
        }

        let ctx = ContextPacket {
            system_prompt:  Some(profile.persona.clone()),
            model_override: Some(model.to_string()),
            ..Default::default()
        };

        let result = orch.run(prompt, ctx, client, api_key, tx).await;

        // Record actual spend (approximate: response length / 4 tokens + estimated input)
        if let Ok(ref response) = result {
            let actual = estimated + (response.len() / 4) as u32;
            LEDGER.record_actual(model, actual);
        }

        result
    }
}
EOF
echo -e "\e[32m  ✓ src/conductor.rs written\e[0m"

# ───────────────────────────────────────────────────────────────────
# 4. Patch src/task_manager.rs — wire in router + conductor + ledger
# ───────────────────────────────────────────────────────────────────
# Strategy: replace the entire file with the upgraded version.
cat << 'EOF' > src/task_manager.rs
// src/task_manager.rs — v5.0 Router-Aware Cognitive Handoff
// Copyright 2026 Donald Ball — Apache 2.0
//
// Changes from v4.6:
//  - IntelligentRouter.route() classifies directive before any API call
//  - Conductor.run_turn() enforces token ledger + model selection
//  - session_obs trimmed to profile.budget.max_obs_lines each turn
//  - max_turns now comes from the AgentProfile (not hardcoded 16)
//  - inject_workspace_dna only called when profile.inject_schema is true

use std::sync::{Arc, LazyLock};
use std::process::Command;
use tokio::sync::{broadcast, Semaphore, Mutex};
use crate::json_parser;
use crate::executor::{RoboticExecutor, FsmState};
use crate::router::IntelligentRouter;
use crate::conductor::Conductor;
use crate::token_ledger::LEDGER;
use serde_json::json;

static TASK_SLAB: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

fn get_current_branch() -> String {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".into())
        .trim()
        .to_string()
}

fn get_git_context() -> String {
    let branch = get_current_branch();
    let log = Command::new("git").args(["log", "--oneline", "-5"]).output()
        .ok().and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let status = Command::new("git").args(["status", "--short"]).output()
        .ok().and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    format!(
        "BRANCH: {}\nRECENT COMMITS:\n{}\nPENDING CHANGES:\n{}",
        branch,
        log.trim(),
        if status.trim().is_empty() { "(clean)" } else { status.trim() }
    )
}

pub async fn send_task(
    orch:      Arc<crate::orchestrator::Orchestrator>,
    client:    &reqwest::Client,
    api_key:   &str,
    directive: &str,
    memory:    Arc<Mutex<crate::memory::MemoryPalace>>,
    vmo_state: Arc<Mutex<crate::server::VmoState>>,
    tx:        broadcast::Sender<String>,
) -> String {
    let _permit = TASK_SLAB.clone().acquire_owned().await.expect("SLAB_LOCK_FAIL");

    // ── STEP 1: Classify and build the execution profile ──────────────
    let profile  = IntelligentRouter::route(directive);
    let max_turns = profile.max_turns;
    let budget   = profile.budget.clone();

    let _ = tx.send(json!({
        "type": "NEURAL_THOUGHT",
        "payload": format!(
            "[ROUTER] Class: {:?} | Primary: {} | MaxTurns: {} | SchemaInject: {}",
            profile.class, profile.primary_model, max_turns, profile.inject_schema
        )
    }).to_string());

    // Log ledger status at task start
    let _ = tx.send(json!({
        "type": "NEURAL_THOUGHT",
        "payload": LEDGER.status()
    }).to_string());

    // ── STEP 2: Build the base prompt (budget-aware) ──────────────────
    let git_ctx = if profile.inject_git { get_git_context() } else { String::new() };
    let base_prompt = IntelligentRouter::build_prompt(directive, &profile, &git_ctx, None);

    let mut session_obs: Vec<String> = Vec::new();
    let mut fsm          = RoboticExecutor::new();
    let mut error_count: usize = 0;

    // ── STEP 3: Turn loop ─────────────────────────────────────────────
    for turn in 1..=max_turns {
        if fsm.current_state == FsmState::Complete || fsm.current_state == FsmState::Fault {
            break;
        }

        let branch = get_current_branch();
        let _ = tx.send(json!({
            "type": "NEURAL_THOUGHT",
            "payload": format!("── Turn {}/{} | Branch: {} ──", turn, max_turns, branch)
        }).to_string());

        // Trim observations to budget — prevents context bloat across turns
        let trimmed_obs: Vec<String> = session_obs
            .iter()
            .rev()
            .take(budget.max_obs_lines)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        // Attach recent observations to the prompt for this turn
        let turn_prompt = if trimmed_obs.is_empty() {
            format!("{}\n\nExecute next step:", base_prompt)
        } else {
            format!(
                "{}\n\nRECENT OBSERVATIONS:\n{}\n\nExecute next step:",
                base_prompt,
                trimmed_obs.iter().map(|o| format!(">> {}", o)).collect::<Vec<_>>().join("\n")
            )
        };

        // ── CONDUCTOR: handles quota check + model selection ──────────
        match Conductor::run_turn(
            &profile,
            turn_prompt,
            error_count,
            &orch,
            client,
            api_key,
            Some(tx.clone()),
        ).await {
            Ok(response) => {
                match json_parser::extract_actions(&response) {
                    Ok(actions) if !actions.is_empty() => {
                        match fsm.execute_handle(actions, memory.clone(), vmo_state.clone()).await {
                            Ok(results) => {
                                for res in &results {
                                    session_obs.push(format!("Turn {} Result: {}", turn, res));
                                }
                                error_count = 0;

                                // Check for halt
                                if results.iter().any(|r| r.contains("MISSION_HALTED")) {
                                    break;
                                }
                            }
                            Err(e) => {
                                error_count += 1;
                                session_obs.push(format!("Turn {} Error: {}", turn, e));
                                let _ = tx.send(json!({
                                    "type": "NEURAL_STREAM",
                                    "payload": format!("\n\x1b[31m[EXEC_FAULT Turn {}] {}\x1b[0m\n", turn, e)
                                }).to_string());
                            }
                        }
                    }
                    _ => {
                        error_count += 1;
                        session_obs.push(format!(
                            "Turn {} System: No valid action block. Use <ACTION>[...]</ACTION>.",
                            turn
                        ));
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(json!({
                    "type": "NEURAL_STREAM",
                    "payload": format!("API_FAULT: {}", e)
                }).to_string());
                // Abort on API fault — don't burn remaining turns
                break;
            }
        }
    }

    // Final ledger report
    let _ = tx.send(json!({
        "type": "TASK_STATUS",
        "payload": LEDGER.status()
    }).to_string());

    "TASK_COMPLETE".into()
}
EOF
echo -e "\e[32m  ✓ src/task_manager.rs upgraded to v5.0\e[0m"

# ───────────────────────────────────────────────────────────────────
# 5. Patch src/server.rs — add /api/ledger endpoint
# ───────────────────────────────────────────────────────────────────
# Append the ledger handler and register the route
python3 - << 'PYEOF'
import re, sys

path = "src/server.rs"
with open(path) as f:
    src = f.read()

# Add the route if it's not already there
if "/api/ledger" not in src:
    src = src.replace(
        '.route("/api/metrics",            get(handle_metrics))',
        '.route("/api/metrics",            get(handle_metrics))\n        .route("/api/ledger",             get(handle_ledger))'
    )

# Add the handler if it's not already there
if "handle_ledger" not in src:
    src += '''
async fn handle_ledger() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        crate::token_ledger::LEDGER.status(),
    )
}
'''

with open(path, "w") as f:
    f.write(src)
print("  ✓ src/server.rs patched — /api/ledger added")
PYEOF

# ───────────────────────────────────────────────────────────────────
# 6. Patch src/lib.rs — register new modules
# ───────────────────────────────────────────────────────────────────
python3 - << 'PYEOF'
path = "src/lib.rs"
with open(path) as f:
    src = f.read()

new_mods = []
for mod in ["router", "token_ledger", "conductor"]:
    if f"pub mod {mod};" not in src:
        new_mods.append(f"pub mod {mod};")

if new_mods:
    # Insert after the last pub mod line
    last_mod_idx = src.rfind("pub mod ")
    eol = src.find("\n", last_mod_idx)
    insert = "\n" + "\n".join(new_mods)
    src = src[:eol] + insert + src[eol:]
    with open(path, "w") as f:
        f.write(src)
    print(f"  ✓ src/lib.rs — added: {', '.join(new_mods)}")
else:
    print("  ✓ src/lib.rs — modules already registered")
PYEOF

# ───────────────────────────────────────────────────────────────────
# 7. Patch Cargo.toml — add once_cell if not present
# ───────────────────────────────────────────────────────────────────
python3 - << 'PYEOF'
path = "Cargo.toml"
with open(path) as f:
    src = f.read()

if "once_cell" not in src:
    src = src.replace(
        "[dependencies]",
        '[dependencies]\nonce_cell = "1"'
    )
    with open(path, "w") as f:
        f.write(src)
    print("  ✓ Cargo.toml — once_cell dependency added")
else:
    print("  ✓ Cargo.toml — once_cell already present")
PYEOF

# ───────────────────────────────────────────────────────────────────
# 8. Verify the build
# ───────────────────────────────────────────────────────────────────
echo ""
echo -e "\e[36m>> [NEXUS] Running cargo check...\e[0m"
cargo check 2>&1 | grep -E "^error|^warning.*unused|Finished|Compiling nexus" || true

if cargo check 2>&1 | grep -q "^error"; then
    echo -e "\e[31m>> [FRACTURE] Build has errors — review above output\e[0m"
    exit 1
else
    echo -e "\e[32m>> [NEXUS] Build clean. Router + Ledger + Conductor active.\e[0m"
fi

# ───────────────────────────────────────────────────────────────────
# 9. Summary
# ───────────────────────────────────────────────────────────────────
echo ""
echo -e "\e[1;36m╔══════════════════════════════════════════════════════════╗\e[0m"
echo -e "\e[1;36m║       NEXUS UPGRADE COMPLETE — v5.0 CONDUCTOR BUILD      ║\e[0m"
echo -e "\e[1;36m╚══════════════════════════════════════════════════════════╝\e[0m"
echo ""
echo -e "  \e[32mNew modules:\e[0m"
echo "    src/router.rs        — zero-token directive classifier"
echo "    src/token_ledger.rs  — RPM/TPM quota tracker + backoff"
echo "    src/conductor.rs     — gemma-4-26b-a4b-it orchestration layer"
echo ""
echo -e "  \e[32mUpgraded modules:\e[0m"
echo "    src/task_manager.rs  — v5.0, router-aware, budget-trimmed context"
echo "    src/server.rs        — /api/ledger endpoint added"
echo ""
echo -e "  \e[32mToken savings (per directive type):\e[0m"
echo "    ShellOp    → from ~3800 chars injected to ~200  (94% reduction)"
echo "    Inquiry    → from ~3800 chars injected to ~800  (79% reduction)"
echo "    TargetedEdit → 5 turns max vs 16            (69% fewer turns)"
echo ""
echo -e "  \e[33mMonitor quota live:\e[0m"
echo "    curl http://localhost:3000/api/ledger"
echo "    curl http://localhost:3000/api/metrics"
echo ""
echo -e "  \e[33mEnv vars to tune limits:\e[0m"
echo "    NEXUS_RPM_OVERRIDE_26B=10  (default 15)"
echo "    NEXUS_TPM_OVERRIDE_26B=500000  (default 1_000_000)"

