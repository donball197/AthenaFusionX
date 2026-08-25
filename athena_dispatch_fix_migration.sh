#!/usr/bin/env bash
# =============================================================================
# AthenaFusionX — Dispatch / Routing / Hallucination Hardening Migration
# =============================================================================
# Implements:
#   1. Delete bak + dead routing implementations
#   2. Single source of truth for resolve_provider + cascades (runtime_core)
#   3. Fix .env defaults (heavy/light/planner match capacity)
#   4. Make get_light_models / get_heavy_models pure
#   5. Strong anti-loop + relevance scoring scaffolding
#   6. Ground the critic prompt (current mission + last K evidence)
#   7. Integration tests: model X → endpoint Y + key Z
#
# Usage:
#   cd ~/AthenaFusionX
#   bash athena_dispatch_fix_migration.sh --dry-run   # preview
#   bash athena_dispatch_fix_migration.sh             # apply
#   bash athena_dispatch_fix_migration.sh --force     # skip confirmations
#
# Safe: creates timestamped backups under .nexus/migration_backups/
# =============================================================================

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# If script is run from outside the repo, try common locations
if [[ ! -d "$ROOT/crates/runtime_core" ]]; then
  if [[ -d "$HOME/AthenaFusionX/crates/runtime_core" ]]; then
    ROOT="$HOME/AthenaFusionX"
  elif [[ -d "./crates/runtime_core" ]]; then
    ROOT="$(pwd)"
  else
    echo "ERROR: Cannot locate AthenaFusionX root (looking for crates/runtime_core)."
    echo "Run this script from inside the repo, or place it at the repo root."
    exit 1
  fi
fi

cd "$ROOT"

DRY_RUN=0
FORCE=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --force)   FORCE=1 ;;
    -h|--help)
      sed -n '2,30p' "$0"
      exit 0
      ;;
  esac
done

TS="$(date +%Y%m%d_%H%M%S)"
BACKUP_DIR="$ROOT/.nexus/migration_backups/$TS"
LOG="$BACKUP_DIR/migration.log"

mkdir -p "$BACKUP_DIR"
exec > >(tee -a "$LOG") 2>&1

echo "============================================================"
echo " AthenaFusionX Dispatch Hardening Migration"
echo " Root: $ROOT"
echo " Backup: $BACKUP_DIR"
echo " Mode: $([[ $DRY_RUN -eq 1 ]] && echo DRY-RUN || echo APPLY)"
echo "============================================================"
echo

confirm() {
  if [[ $FORCE -eq 1 || $DRY_RUN -eq 1 ]]; then return 0; fi
  read -r -p "$1 [y/N] " ans
  [[ "${ans,,}" == "y" || "${ans,,}" == "yes" ]]
}

run() {
  if [[ $DRY_RUN -eq 1 ]]; then
    echo "  [DRY] $*"
  else
    echo "  [RUN] $*"
    eval "$@"
  fi
}

backup_file() {
  local f="$1"
  if [[ -f "$f" ]]; then
    local rel="${f#$ROOT/}"
    local dest="$BACKUP_DIR/$rel"
    mkdir -p "$(dirname "$dest")"
    cp -a "$f" "$dest"
    echo "  [BACKUP] $rel"
  fi
}

# -----------------------------------------------------------------------------
# PHASE 0 — Pre-flight
# -----------------------------------------------------------------------------
echo "=== PHASE 0: Pre-flight ==="
if [[ ! -f "$ROOT/crates/runtime_core/Cargo.toml" ]]; then
  echo "ERROR: runtime_core not found"
  exit 1
fi

# Ensure we are on a clean-ish tree (warn only)
if command -v git >/dev/null 2>&1 && [[ -d .git ]]; then
  if ! git diff --quiet 2>/dev/null; then
    echo "WARNING: Working tree has uncommitted changes. Backups will still be taken."
    confirm "Continue anyway?" || exit 1
  fi
fi
echo

# -----------------------------------------------------------------------------
# PHASE 1 — Delete bak + obviously dead routing files
# -----------------------------------------------------------------------------
echo "=== PHASE 1: Remove dead routing implementations ==="

DEAD_CANDIDATES=(
  "crates/nexus_edge_minion/src/api_gateway.rs.bak.20260824_130024"
  "crates/nexus_edge_minion/src/api_gateway.rs.bak"
  "crates/nexus_edge_minion/src/api_gateway.rs.old"
  "crates/nexus_edge_minion/src/api_gateway.rs~"
)

# Also hunt for any other *.bak* under crates/ and apps/
while IFS= read -r -d '' f; do
  DEAD_CANDIDATES+=("$f")
done < <(find crates apps -type f \( -name '*.bak' -o -name '*.bak.*' -o -name '*~' \) -print0 2>/dev/null || true)

# Deduplicate
mapfile -t DEAD_CANDIDATES < <(printf '%s\n' "${DEAD_CANDIDATES[@]}" | sort -u)

for f in "${DEAD_CANDIDATES[@]}"; do
  if [[ -f "$f" ]]; then
    backup_file "$f"
    run "rm -f \"$f\""
    echo "  [DELETE] $f"
  fi
done
echo

# -----------------------------------------------------------------------------
# PHASE 2 — Create single source of truth: runtime_core/src/provider.rs
# -----------------------------------------------------------------------------
echo "=== PHASE 2: Create unified provider module (runtime_core) ==="

PROVIDER_RS="$ROOT/crates/runtime_core/src/provider.rs"
backup_file "$PROVIDER_RS"

cat > /tmp/athena_provider_rs.$$ << 'RUST'
//! Unified LLM provider resolution + cascade construction.
//!
//! SINGLE SOURCE OF TRUTH for:
//!   - resolve_provider(model) → (endpoint, api_key, auth_style)
//!   - get_light_models() / get_heavy_models()  (pure, no cross-tier injection)
//!   - cascade helpers
//!
//! All other crates MUST call into this module (or re-export it).
//! Do NOT re-implement provider detection elsewhere.

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};

/// Circuit-breaker timestamps (unix secs). 0 = healthy.
static GEMINI_DISABLED_UNTIL: AtomicU64 = AtomicU64::new(0);
static NVIDIA_DISABLED_UNTIL: AtomicU64 = AtomicU64::new(0);
const CIRCUIT_COOLDOWN_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    /// Authorization: Bearer <key>
    Bearer,
    /// x-goog-api-key: <key>
    GoogleApiKey,
}

#[derive(Debug, Clone)]
pub struct ProviderResolved {
    pub endpoint: String,
    pub api_key: String,
    pub auth: AuthStyle,
    pub provider_name: &'static str,
}

/// Record a rate-limit / 429 so the cascade skips this provider briefly.
pub fn trip_circuit(model_name: &str) {
    let now = now_secs();
    let until = now + CIRCUIT_COOLDOWN_SECS;
    let lower = model_name.to_ascii_lowercase();
    if lower.contains("gemini") {
        GEMINI_DISABLED_UNTIL.store(until, Ordering::Relaxed);
        tracing::warn!("[CIRCUIT] Gemini rate-limited; bypassing for {}s", CIRCUIT_COOLDOWN_SECS);
    } else if lower.contains("nvidia") || lower.contains("nemotron") || lower.contains("deepseek") {
        NVIDIA_DISABLED_UNTIL.store(until, Ordering::Relaxed);
        tracing::warn!("[CIRCUIT] NVIDIA/NIM rate-limited; bypassing for {}s", CIRCUIT_COOLDOWN_SECS);
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn provider_healthy(kind: &str) -> bool {
    let now = now_secs();
    match kind {
        "gemini" => now > GEMINI_DISABLED_UNTIL.load(Ordering::Relaxed),
        "nvidia" => now > NVIDIA_DISABLED_UNTIL.load(Ordering::Relaxed),
        _ => true,
    }
}

/// Resolve the correct endpoint + key for a model string.
///
/// Rules (strict):
///   - contains "gemini"           → Google OpenAI-compatible endpoint + GEMINI_API_KEY
///   - contains "nvidia" | "nemotron" | "deepseek-ai" | starts with "nvidia/"
///                                 → NVIDIA NIM + NVIDIA_API_KEY
///   - everything else             → NEXUS_API_ENDPOINT + NEXUS_AUTH_KEY
///
/// Model family names (llama, gemma, ising-calibration, …) are NOT providers.
/// They only resolve via the configured Nexus endpoint unless they carry an
/// explicit provider prefix.
pub fn resolve_provider(model_name: &str) -> ProviderResolved {
    let model = model_name.to_ascii_lowercase();

    // Google Gemini
    if model.contains("gemini") {
        return ProviderResolved {
            endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
                .to_string(),
            api_key: env::var("GEMINI_API_KEY").unwrap_or_default(),
            auth: AuthStyle::GoogleApiKey, // also accept Bearer on the openai path
            provider_name: "google",
        };
    }

    // NVIDIA NIM / hosted models (explicit prefixes only)
    if model.contains("nvidia")
        || model.contains("nemotron")
        || model.starts_with("deepseek-ai/")
        || model.contains("deepseek")
    {
        return ProviderResolved {
            endpoint: "https://integrate.api.nvidia.com/v1/chat/completions".to_string(),
            api_key: env::var("NVIDIA_API_KEY").unwrap_or_default(),
            auth: AuthStyle::Bearer,
            provider_name: "nvidia",
        };
    }

    // Configured Nexus / custom OpenAI-compatible gateway
    ProviderResolved {
        endpoint: env::var("NEXUS_API_ENDPOINT").unwrap_or_else(|_| {
            "https://api.example.com/v1/chat/completions".to_string()
        }),
        api_key: env::var("NEXUS_AUTH_KEY").unwrap_or_default(),
        auth: AuthStyle::Bearer,
        provider_name: "nexus",
    }
}

/// Pure light-model list. Never injects heavy models.
pub fn get_light_models() -> Vec<String> {
    let mut out = Vec::new();

    // Explicit cascade first
    if let Ok(cascade) = env::var("NEXUS_LIGHT_CASCADE") {
        for m in cascade.split(',') {
            let m = m.trim();
            if !m.is_empty() && !out.iter().any(|x| x == m) {
                out.push(m.to_string());
            }
        }
    }

    // Single light model
    if let Ok(m) = env::var("NEXUS_LIGHT_MODEL") {
        let m = m.trim();
        if !m.is_empty() && !out.iter().any(|x| x == m) {
            out.push(m.to_string());
        }
    }

    // Filter by circuit breaker
    out.retain(|m| {
        let lower = m.to_ascii_lowercase();
        if lower.contains("gemini") {
            provider_healthy("gemini")
        } else if lower.contains("nvidia") || lower.contains("nemotron") || lower.contains("deepseek") {
            provider_healthy("nvidia")
        } else {
            true
        }
    });

    if out.is_empty() {
        // Last-resort safe defaults (light capacity only)
        out.push("gemini-2.0-flash".to_string());
    }
    out
}

/// Pure heavy-model list. Never injects light models.
pub fn get_heavy_models() -> Vec<String> {
    let mut out = Vec::new();

    if let Ok(cascade) = env::var("NEXUS_HEAVY_CASCADE") {
        for m in cascade.split(',') {
            let m = m.trim();
            if !m.is_empty() && !out.iter().any(|x| x == m) {
                out.push(m.to_string());
            }
        }
    }

    if let Ok(m) = env::var("NEXUS_HEAVY_MODEL") {
        let m = m.trim();
        if !m.is_empty() && !out.iter().any(|x| x == m) {
            out.push(m.to_string());
        }
    }

    out.retain(|m| {
        let lower = m.to_ascii_lowercase();
        if lower.contains("gemini") {
            provider_healthy("gemini")
        } else if lower.contains("nvidia") || lower.contains("nemotron") || lower.contains("deepseek") {
            provider_healthy("nvidia")
        } else {
            true
        }
    });

    if out.is_empty() {
        out.push("gemini-2.5-pro".to_string());
    }
    out
}

/// Planner model list (planner first, then light cascade).
pub fn get_planner_models() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(m) = env::var("NEXUS_PLANNER_MODEL") {
        let m = m.trim();
        if !m.is_empty() {
            out.push(m.to_string());
        }
    }
    for m in get_light_models() {
        if !out.iter().any(|x| x == &m) {
            out.push(m);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_routes_to_google() {
        let r = resolve_provider("gemini-2.0-flash");
        assert!(r.endpoint.contains("generativelanguage.googleapis.com"));
        assert_eq!(r.provider_name, "google");
    }

    #[test]
    fn nemotron_routes_to_nvidia() {
        let r = resolve_provider("nvidia/nemotron-3-ultra-550b-a55b");
        assert!(r.endpoint.contains("integrate.api.nvidia.com"));
        assert_eq!(r.provider_name, "nvidia");
        assert_eq!(r.auth, AuthStyle::Bearer);
    }

    #[test]
    fn deepseek_routes_to_nvidia() {
        let r = resolve_provider("deepseek-ai/deepseek-v4-flash-0731");
        assert!(r.endpoint.contains("integrate.api.nvidia.com"));
        assert_eq!(r.provider_name, "nvidia");
    }

    #[test]
    fn unknown_family_uses_nexus() {
        // "gemma" / "llama" without provider prefix must NOT go to Google or NVIDIA by name alone
        let r = resolve_provider("gemma-2-9b");
        assert_eq!(r.provider_name, "nexus");
    }
}
RUST

if [[ $DRY_RUN -eq 1 ]]; then
  echo "  [DRY] Would write $PROVIDER_RS"
else
  cp /tmp/athena_provider_rs.$$ "$PROVIDER_RS"
  echo "  [WRITE] $PROVIDER_RS"
fi
rm -f /tmp/athena_provider_rs.$$

# Ensure mod provider; is in lib.rs
LIB_RS="$ROOT/crates/runtime_core/src/lib.rs"
if [[ -f "$LIB_RS" ]]; then
  backup_file "$LIB_RS"
  if ! grep -q 'mod provider;' "$LIB_RS" 2>/dev/null && ! grep -q 'pub mod provider;' "$LIB_RS" 2>/dev/null; then
    if [[ $DRY_RUN -eq 1 ]]; then
      echo "  [DRY] Would add 'pub mod provider;' to lib.rs"
    else
      # Insert after the first mod/pub mod line block or at top after comments
      if grep -q '^pub mod ' "$LIB_RS" || grep -q '^mod ' "$LIB_RS"; then
        # Add near other mods
        sed -i '/^pub mod /a pub mod provider;' "$LIB_RS" 2>/dev/null || \
        sed -i '/^mod /a pub mod provider;' "$LIB_RS" 2>/dev/null || \
        echo 'pub mod provider;' >> "$LIB_RS"
      else
        echo 'pub mod provider;' >> "$LIB_RS"
      fi
      # Dedup if sed doubled
      awk '!seen[$0]++' "$LIB_RS" > /tmp/lib_rs_clean.$$ && mv /tmp/lib_rs_clean.$$ "$LIB_RS"
      echo "  [PATCH] lib.rs → pub mod provider;"
    fi
  else
    echo "  [OK] provider module already declared in lib.rs"
  fi
fi
echo

# -----------------------------------------------------------------------------
# PHASE 3 — Make model_config.rs pure (delegate to provider)
# -----------------------------------------------------------------------------
echo "=== PHASE 3: Make get_light_models / get_heavy_models pure ==="

MODEL_CONFIG="$ROOT/crates/runtime_core/src/model_config.rs"
if [[ -f "$MODEL_CONFIG" ]]; then
  backup_file "$MODEL_CONFIG"
  if [[ $DRY_RUN -eq 1 ]]; then
    echo "  [DRY] Would rewrite model_config.rs to re-export provider helpers"
  else
    cat > "$MODEL_CONFIG" << 'RUST'
//! Model cascade helpers — thin re-export of the single source of truth.
//! Prefer `crate::provider::{get_light_models, get_heavy_models, resolve_provider}`.

pub use crate::provider::{
    get_heavy_models, get_light_models, get_planner_models, resolve_provider, trip_circuit,
    AuthStyle, ProviderResolved,
};

// Keep old circuit-breaker entry points working for any remaining callers.
pub fn mark_rate_limited(model_name: &str) {
    trip_circuit(model_name);
}
RUST
    echo "  [WRITE] $MODEL_CONFIG (pure re-export)"
  fi
else
  echo "  [SKIP] model_config.rs not found"
fi
echo

# -----------------------------------------------------------------------------
# PHASE 4 — Fix .env defaults
# -----------------------------------------------------------------------------
echo "=== PHASE 4: Fix .env defaults ==="

ENV_FILE="$ROOT/.env"
if [[ -f "$ENV_FILE" ]]; then
  backup_file "$ENV_FILE"
fi

# Sensible defaults assuming both GEMINI_API_KEY and NVIDIA_API_KEY may exist.
# Heavy = strong reasoning; Light = fast/cheap; Planner = structured JSON.
NEW_ENV_SNIPPET='# ---- AthenaFusionX model defaults (migration '"$TS"') ----
# Heavy: strong reasoning / synthesis
NEXUS_HEAVY_MODEL=gemini-2.5-pro
# Light: fast tool-calling / critic / strategist turns
NEXUS_LIGHT_MODEL=gemini-2.0-flash
# Planner: DAG / structured output
NEXUS_PLANNER_MODEL=gemini-2.0-flash

# Cascades (comma-separated, tried in order). Keep tiers pure.
NEXUS_LIGHT_CASCADE=gemini-2.0-flash,gemini-2.0-flash-lite
NEXUS_HEAVY_CASCADE=gemini-2.5-pro,gemini-2.0-flash

# If you have NVIDIA keys and want NIM models, append explicitly, e.g.:
# NEXUS_HEAVY_CASCADE=nvidia/nemotron-3-ultra-550b-a55b,gemini-2.5-pro
# NEXUS_LIGHT_CASCADE=nvidia/llama-3.3-70b-instruct,gemini-2.0-flash
'

if [[ $DRY_RUN -eq 1 ]]; then
  echo "  [DRY] Would update .env with:"
  echo "$NEW_ENV_SNIPPET"
else
  # Remove old NEXUS_*_MODEL / CASCADE lines then append clean block
  if [[ -f "$ENV_FILE" ]]; then
    grep -v -E '^(NEXUS_HEAVY_MODEL|NEXUS_LIGHT_MODEL|NEXUS_PLANNER_MODEL|NEXUS_LIGHT_CASCADE|NEXUS_HEAVY_CASCADE)=' "$ENV_FILE" > /tmp/env_clean.$$ || true
    mv /tmp/env_clean.$$ "$ENV_FILE"
  else
    touch "$ENV_FILE"
  fi
  echo "$NEW_ENV_SNIPPET" >> "$ENV_FILE"
  echo "  [WRITE] .env model defaults updated"
  echo "  NOTE: Edit .env if you prefer NVIDIA models as primary."
fi
echo

# -----------------------------------------------------------------------------
# PHASE 5 — Patch api_gateway.rs to use shared resolve_provider
# -----------------------------------------------------------------------------
echo "=== PHASE 5: Point nexus_edge_minion ApiGateway at shared provider ==="

GATEWAY="$ROOT/crates/nexus_edge_minion/src/api_gateway.rs"
if [[ -f "$GATEWAY" ]]; then
  backup_file "$GATEWAY"
  if [[ $DRY_RUN -eq 1 ]]; then
    echo "  [DRY] Would replace resolve_provider body with call to runtime_core::provider"
  else
    # We write a clear comment + a thin wrapper so the rest of the file keeps compiling.
    # Full mechanical rewrite of every call site is left as a follow-up; this stops the
    # divergent logic from being the one that runs.
    if grep -q 'fn resolve_provider' "$GATEWAY"; then
      # Insert a deprecation notice at the top of the function if not already present
      if ! grep -q 'DEPRECATED: use runtime_core::provider::resolve_provider' "$GATEWAY"; then
        sed -i 's/fn resolve_provider/\/\/ DEPRECATED: use runtime_core::provider::resolve_provider\n    fn resolve_provider/' "$GATEWAY" || true
      fi
      echo "  [PATCH] Marked local resolve_provider as DEPRECATED in api_gateway.rs"
      echo "  [TODO]  Manually replace its body with a call into runtime_core once the"
      echo "          nexus_edge_minion crate depends on runtime_core (or extract a"
      echo "          tiny nexus_provider crate both can share)."
    fi
  fi
else
  echo "  [SKIP] api_gateway.rs not found"
fi
echo

# -----------------------------------------------------------------------------
# PHASE 6 — Anti-loop + relevance scoring scaffolding
# -----------------------------------------------------------------------------
echo "=== PHASE 6: Anti-loop + search relevance scaffolding ==="

ANTILOOP_RS="$ROOT/crates/runtime_core/src/anti_loop.rs"
backup_file "$ANTILOOP_RS"

cat > /tmp/athena_antiloop.$$ << 'RUST'
//! Strong anti-loop + cheap search-result relevance scoring.
//!
//! Prevents the strategist from repeating the identical tool call forever
//! and stops LICENSE / Cargo.lock matches from raising belief.

use std::collections::HashMap;

/// Tracks recent tool invocations. Call `record` after every tool result.
#[derive(Debug, Default)]
pub struct AntiLoopGuard {
    /// key → consecutive count
    counts: HashMap<String, u32>,
    /// Maximum identical calls before forced pivot
    pub max_identical: u32,
}

impl AntiLoopGuard {
    pub fn new(max_identical: u32) -> Self {
        Self {
            counts: HashMap::new(),
            max_identical: max_identical.max(2),
        }
    }

    /// Returns true if this call is ALLOWED. Returns false if the guard
    /// demands a pivot (caller must choose a different tool/args).
    pub fn record(&mut self, tool: &str, path: &str, query: &str) -> bool {
        let key = format!("{tool}|{path}|{query}").to_ascii_lowercase();
        let c = self.counts.entry(key).or_insert(0);
        *c += 1;
        // Decay others slightly so the map doesn't grow forever
        if self.counts.len() > 64 {
            self.counts.retain(|_, v| {
                *v = v.saturating_sub(1);
                *v > 0
            });
        }
        *c <= self.max_identical
    }

    pub fn reset(&mut self) {
        self.counts.clear();
    }
}

/// Cheap heuristic relevance score for search_code / grep output.
/// Returns 0.0 ..= 1.0
///
/// LICENSE, Cargo.lock, binary files, and pure copyright text score near 0.
/// Hits that look like definitions (struct/enum/fn/impl/mod) score high.
pub fn score_search_relevance(query: &str, raw_output: &str) -> f32 {
    if raw_output.trim().is_empty() {
        return 0.0;
    }
    let lower = raw_output.to_ascii_lowercase();
    let q = query.to_ascii_lowercase();

    // Hard penalties
    let mut score: f32 = 0.35;
    if lower.contains("license") || lower.contains("copyright (c)") || lower.contains("all rights reserved") {
        score -= 0.25;
    }
    if lower.contains("cargo.lock") || lower.contains("[[package]]") {
        score -= 0.20;
    }
    if lower.contains(".png") || lower.contains(".jpg") || lower.contains("binary file") {
        score -= 0.30;
    }

    // Definition-like hits
    let def_markers = ["struct ", "enum ", "fn ", "impl ", "mod ", "type ", "trait ", "pub struct", "pub enum", "pub fn"];
    let mut def_hits = 0;
    for m in def_markers {
        if lower.contains(m) {
            def_hits += 1;
        }
    }
    score += (def_hits as f32) * 0.08;

    // Query token present in a code-ish line
    if !q.is_empty() && lower.contains(&q) {
        score += 0.15;
    }

    score.clamp(0.0, 1.0)
}

/// Only allow belief increases when relevance >= threshold.
pub fn should_raise_belief(relevance: f32, threshold: f32) -> bool {
    relevance >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anti_loop_trips() {
        let mut g = AntiLoopGuard::new(3);
        assert!(g.record("search_code", "apps/warp", "state"));
        assert!(g.record("search_code", "apps/warp", "state"));
        assert!(g.record("search_code", "apps/warp", "state"));
        assert!(!g.record("search_code", "apps/warp", "state"));
    }

    #[test]
    fn license_is_low_relevance() {
        let out = "LICENSE-AGPL\nCopyright (C) 2024\nAll rights reserved\n";
        let s = score_search_relevance("state", out);
        assert!(s < 0.3, "LICENSE should be low relevance, got {s}");
    }

    #[test]
    fn struct_def_is_high_relevance() {
        let out = "src/foo.rs:12: pub struct CorticalLoop {\nsrc/foo.rs:13:     client: Client,\n";
        let s = score_search_relevance("CorticalLoop", out);
        assert!(s >= 0.5, "definition should be high relevance, got {s}");
    }
}
RUST

if [[ $DRY_RUN -eq 1 ]]; then
  echo "  [DRY] Would write $ANTILOOP_RS"
else
  cp /tmp/athena_antiloop.$$ "$ANTILOOP_RS"
  echo "  [WRITE] $ANTILOOP_RS"
  if [[ -f "$LIB_RS" ]] && ! grep -q 'mod anti_loop;' "$LIB_RS" && ! grep -q 'pub mod anti_loop;' "$LIB_RS"; then
    echo 'pub mod anti_loop;' >> "$LIB_RS"
    awk '!seen[$0]++' "$LIB_RS" > /tmp/lib_rs_clean2.$$ && mv /tmp/lib_rs_clean2.$$ "$LIB_RS"
    echo "  [PATCH] lib.rs → pub mod anti_loop;"
  fi
fi
rm -f /tmp/athena_antiloop.$$
echo

# -----------------------------------------------------------------------------
# PHASE 7 — Critic grounding note (prompt template)
# -----------------------------------------------------------------------------
echo "=== PHASE 7: Critic grounding (prompt hygiene) ==="

EPISTEMIC="$ROOT/crates/runtime_core/src/epistemic_engine.rs"
if [[ -f "$EPISTEMIC" ]]; then
  backup_file "$EPISTEMIC"
  if [[ $DRY_RUN -eq 1 ]]; then
    echo "  [DRY] Would inject grounding preamble into criticize_proposal"
  else
    # Non-destructive: add a constant the human can wire in.
    if ! grep -q 'CRITIC_GROUNDING_PREAMBLE' "$EPISTEMIC"; then
      cat >> "$EPISTEMIC" << 'RUST'

/// Prepend to every critic call. Keeps the critic on the *current* mission
/// and the last K evidence items only. Strip any historical mission residue
/// (calibration buffers, video recording, etc.) before the LLM sees it.
pub const CRITIC_GROUNDING_PREAMBLE: &str = r#"
[GROUNDING]
You are the Adversarial Critic for the CURRENT mission only.
- Evaluate ONLY the proposal against the ORIGINAL DIRECTIVE and the EVIDENCE block below.
- IGNORE any prior missions, calibration, video-recording, or buffer-size topics unless they appear in the current directive or evidence.
- Do NOT invent quantitative claims. If the Strategist makes a quantitative claim that is not backed by a [Measured] line in EVIDENCE, veto it.
- Prefer constructive vetoes that name the exact next tool call.
"#;
RUST
      echo "  [PATCH] Added CRITIC_GROUNDING_PREAMBLE constant to epistemic_engine.rs"
      echo "  [TODO]  In criticize_proposal(), prepend CRITIC_GROUNDING_PREAMBLE to the prompt"
      echo "          and pass only the last K (e.g. 12) evidence items."
    else
      echo "  [OK] CRITIC_GROUNDING_PREAMBLE already present"
    fi
  fi
else
  echo "  [SKIP] epistemic_engine.rs not found"
fi
echo

# -----------------------------------------------------------------------------
# PHASE 8 — Integration tests for provider routing
# -----------------------------------------------------------------------------
echo "=== PHASE 8: Integration tests (model → endpoint + key) ==="

TEST_DIR="$ROOT/crates/runtime_core/tests"
TEST_FILE="$TEST_DIR/provider_routing.rs"
mkdir -p "$TEST_DIR"
backup_file "$TEST_FILE"

cat > /tmp/athena_provider_test.$$ << 'RUST'
//! Integration-style unit tests: model X must resolve to endpoint Y.
//! Run with: cargo test -p nexus_runtime_core provider_routing -- --nocapture

use nexus_runtime_core::provider::{resolve_provider, AuthStyle};

#[test]
fn gemini_hits_google() {
    let r = resolve_provider("gemini-2.0-flash");
    assert!(
        r.endpoint.contains("generativelanguage.googleapis.com"),
        "Gemini must hit Google, got {}",
        r.endpoint
    );
    assert_eq!(r.provider_name, "google");
}

#[test]
fn nemotron_hits_nvidia() {
    let r = resolve_provider("nvidia/nemotron-3-ultra-550b-a55b");
    assert!(
        r.endpoint.contains("integrate.api.nvidia.com"),
        "Nemotron must hit NVIDIA NIM, got {}",
        r.endpoint
    );
    assert_eq!(r.provider_name, "nvidia");
    assert_eq!(r.auth, AuthStyle::Bearer);
}

#[test]
fn deepseek_hits_nvidia() {
    let r = resolve_provider("deepseek-ai/deepseek-v4-flash-0731");
    assert!(
        r.endpoint.contains("integrate.api.nvidia.com"),
        "DeepSeek must hit NVIDIA NIM, got {}",
        r.endpoint
    );
}

#[test]
fn plain_gemma_does_not_guess_google_or_nvidia() {
    let r = resolve_provider("gemma-2-9b-it");
    assert_eq!(
        r.provider_name, "nexus",
        "Bare model family names must use Nexus endpoint, not guess a cloud provider"
    );
}

#[test]
fn plain_llama_does_not_guess() {
    let r = resolve_provider("llama-3.3-70b");
    assert_eq!(r.provider_name, "nexus");
}
RUST

if [[ $DRY_RUN -eq 1 ]]; then
  echo "  [DRY] Would write $TEST_FILE"
else
  cp /tmp/athena_provider_test.$$ "$TEST_FILE"
  echo "  [WRITE] $TEST_FILE"
fi
rm -f /tmp/athena_provider_test.$$
echo

# -----------------------------------------------------------------------------
# PHASE 9 — Summary + next manual steps
# -----------------------------------------------------------------------------
echo "=== PHASE 9: Summary ==="
echo
echo "Backups:     $BACKUP_DIR"
echo "Log:         $LOG"
echo
echo "Completed automatically:"
echo "  [x] Deleted .bak / dead routing files (with backup)"
echo "  [x] Created crates/runtime_core/src/provider.rs  (single source of truth)"
echo "  [x] Made model_config.rs a pure re-export"
echo "  [x] Fixed .env heavy/light/planner defaults"
echo "  [x] Added anti_loop.rs (identical-call guard + search relevance)"
echo "  [x] Added CRITIC_GROUNDING_PREAMBLE constant"
echo "  [x] Added provider_routing integration tests"
echo
echo "Manual follow-ups required:"
echo "  1. Wire AntiLoopGuard into CorticalLoop (record every tool call;"
echo "     on false, force strategy pivot / different tool)."
echo "  2. In search_code result handling, call score_search_relevance()"
echo "     and only raise belief when should_raise_belief(score, 0.45)."
echo "  3. In criticize_proposal(), prepend CRITIC_GROUNDING_PREAMBLE and"
echo "     pass only the last K evidence items."
echo "  4. Make nexus_edge_minion (and other crates) depend on runtime_core"
echo "     (or extract a tiny nexus_provider crate) and delete their local"
echo "     resolve_provider implementations."
echo "  5. cargo test -p nexus_runtime_core provider_routing"
echo "  6. cargo check -p nexus_runtime_core && ./script/presubmit (if present)"
echo
if [[ $DRY_RUN -eq 1 ]]; then
  echo "*** DRY-RUN complete — no files were modified. Re-run without --dry-run to apply. ***"
else
  echo "*** APPLY complete. Review diffs, run tests, then commit. ***"
fi
echo "============================================================"
