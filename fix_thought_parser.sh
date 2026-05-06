#!/usr/bin/env bash
# =============================================================================
# FIX 1: json_parser.rs — Strip Gemma "thought" blocks before parsing
#
# ROOT CAUSE: Gemma 4 returns two content parts in its response:
#   Part 1: { "text": "..reasoning..", "thought": true }  <-- preamble/thinking
#   Part 2: { "text": "[{\"type\":\"read_file\",...}]" }  <-- actual JSON action
#
# Your neural_agent.rs concatenates ALL parts into one string, so the parser
# sees "The user wants me to...[{...}]" and can never reliably find the array.
#
# THE FIX: In json_parser.rs, scan for the LAST '[' that looks like the start
# of a JSON action array (i.e. followed by '{'), rather than the FIRST '['.
# This skips over any thought text and lands on the real payload.
# =============================================================================
set -euo pipefail

REPO_ROOT="${1:-.}"
cd "$REPO_ROOT"

if [[ ! -f "src/json_parser.rs" ]]; then
    echo "!! [ABORT] src/json_parser.rs not found. Run from repo root."
    exit 1
fi

echo ">> [FIX 1] Patching src/json_parser.rs..."
cp src/json_parser.rs src/json_parser.rs.bak

python3 - <<'PYEOF'
path = "src/json_parser.rs"
with open(path) as f:
    src = f.read()

# Replace the fragile first-'[' search with a last-valid-array-start search
old_find = '''\
    let start = renamed.find('[')
        .ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array start '[' found"))?;
    let end = renamed.rfind(']')
        .ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array end ']' found"))?;

    if start >= end {
        return Err(anyhow!("PARSER_FRACTURE: Invalid JSON array bounds"));
    }'''

new_find = '''\
    // PATCH: Gemma 4 prepends a "thought" block before the real JSON array.
    // Find the LAST '[' that is immediately followed by optional whitespace and '{'
    // so we skip over any reasoning text and land on the actual action array.
    let start = {
        let bytes = renamed.as_bytes();
        let mut found = None;
        let mut i = renamed.len().saturating_sub(1);
        loop {
            if bytes[i] == b\'[\' {
                // Check if this bracket opens a JSON object array
                let rest = renamed[i+1..].trim_start();
                if rest.starts_with(\'{\') || rest.starts_with(\']\') {
                    found = Some(i);
                    break;
                }
            }
            if i == 0 { break; }
            i -= 1;
        }
        found.ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array start \'[\' found"))?
    };
    let end = renamed.rfind(\']\')
        .ok_or_else(|| anyhow!("PARSER_FRACTURE: No JSON array end \']\' found"))?;

    if start >= end {
        return Err(anyhow!("PARSER_FRACTURE: Invalid JSON array bounds"));
    }'''

if old_find not in src:
    print("!! [WARN] Target block not found — already patched or code has drifted.")
    print("   Check src/json_parser.rs manually.")
else:
    src = src.replace(old_find, new_find, 1)
    with open(path, "w") as f:
        f.write(src)
    print("   src/json_parser.rs patched successfully.")
PYEOF

echo ">> [FIX 1] Done."
