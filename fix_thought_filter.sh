#!/usr/bin/env bash
# =============================================================================
# FIX 2: neural_agent.rs — Filter out "thought" parts at the source
#
# This is the cleaner, more permanent fix. Instead of letting the thought block
# bleed into the response string at all, we filter it out right where the API
# response is parsed — before the text ever reaches json_parser.
#
# Gemma 4 response structure:
#   candidates[0].content.parts = [
#     { "text": "...thinking...", "thought": true },   <- SKIP THIS
#     { "text": "[{action json}]" }                    <- USE THIS
#   ]
#
# We patch the response-assembly loop in neural_agent.rs to skip any part
# where "thought" == true.
# =============================================================================
set -euo pipefail

REPO_ROOT="${1:-.}"
cd "$REPO_ROOT"

if [[ ! -f "src/neural_agent.rs" ]]; then
    echo "!! [ABORT] src/neural_agent.rs not found. Run from repo root."
    exit 1
fi

echo ">> [FIX 2] Patching src/neural_agent.rs to filter thought blocks..."
cp src/neural_agent.rs src/neural_agent.rs.bak

python3 - <<'PYEOF'
path = "src/neural_agent.rs"
with open(path) as f:
    src = f.read()

# ── Strategy: find where parts are iterated and text is collected ──────────
# The typical pattern in streaming Gemini clients is something like:
#   for part in parts { if let Some(text) = part["text"].as_str() { ... } }
# We need to add a guard: skip parts where part["thought"] == true.

# Pattern A — common non-streaming pattern collecting parts into a string
old_a = '                if let Some(text) = part["text"].as_str() {'
new_a = '''\
                // PATCH: skip Gemma 4 "thought" blocks — they are internal
                // reasoning and must not be fed into the JSON action parser.
                if part["thought"].as_bool().unwrap_or(false) {
                    continue;
                }
                if let Some(text) = part["text"].as_str() {'''

# Pattern B — alternative formulation sometimes used
old_b = '                    if let Some(t) = part["text"].as_str() {'
new_b = '''\
                    // PATCH: skip Gemma 4 "thought" blocks
                    if part["thought"].as_bool().unwrap_or(false) {
                        continue;
                    }
                    if let Some(t) = part["text"].as_str() {'''

patched = False

if old_a in src:
    src = src.replace(old_a, new_a, 1)
    patched = True
    print("   Patched pattern A in src/neural_agent.rs")

if old_b in src:
    src = src.replace(old_b, new_b, 1)
    patched = True
    print("   Patched pattern B in src/neural_agent.rs")

if not patched:
    print("!! [WARN] Could not find exact part-iteration pattern.")
    print("   The response assembly loop looks different than expected.")
    print("   Search src/neural_agent.rs for where part[\"text\"] is read")
    print("   and add this guard before it:")
    print()
    print('     if part["thought"].as_bool().unwrap_or(false) { continue; }')
    print()
    print("   Backup saved as src/neural_agent.rs.bak")
else:
    with open(path, "w") as f:
        f.write(src)
    print("   src/neural_agent.rs patched successfully.")
PYEOF

echo ">> [FIX 2] Done."
