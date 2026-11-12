#!/usr/bin/env bash
# patch_gate_auto_halt.sh — gate auto-halt on proposal sanity.
#
# Bug: when extract_actions rejects a proposal (e.g. control-token leak such as
# `<|end|><|start|>assistant to=execute_command`), `actions` is left empty.
# The auto-halt branch fires on "empty actions + Satisfied status" and
# synthesizes a halt, overriding the model's intent (it wanted to run
# `execute_command`). The parse-fault recovery path — which would push
# `[PARSE_FAULT]` to evidence and let the strategist retry — never runs.
#
# Fix: detect malformed proposals from their wire form and skip auto-halt
# when one is present. The strategist sees `[PARSE_FAULT]` on the next turn
# and can retry with cleaner serialization.
#
# Usage:
#   ./patch_gate_auto_halt.sh            # dry-run
#   ./patch_gate_auto_halt.sh --apply    # apply + targeted tests

set -euo pipefail

ROOT="$(pwd)"
SRC="$ROOT/crates/runtime_core/src/cortical_loop/strategize.rs"
APPLY=0
FULL=0

for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --full)  FULL=1 ;;
    -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

[[ -f "$SRC" ]] || { echo "ERROR: $SRC not found" >&2; exit 1; }

STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$SRC.bak.$STAMP"
cp "$SRC" "$BACKUP"
echo "backup: $BACKUP"

PATCHED="$(mktemp)"
trap 'rm -f "$PATCHED"' EXIT

python3 - "$SRC" "$PATCHED" <<'PY'
import sys

src_path, out_path = sys.argv[1:3]
src = open(src_path, encoding='utf-8').read()

if 'fn proposal_was_malformed' in src:
    sys.stderr.write("ERROR: proposal_was_malformed already exists\n")
    sys.exit(3)

# ---- 1. Insert the helper before `impl CorticalLoop {` (first occurrence).
anchor = "impl CorticalLoop {\n    pub(super) async fn handle_strategize("
if anchor not in src:
    sys.stderr.write("ERROR: impl CorticalLoop { handle_strategize } anchor not found\n")
    sys.exit(3)

helper = '''/// True when the strategist's raw proposal wire form contains control-token
/// leak markers. Such proposals fail `json_parser::extract_actions` and leave
/// `actions` empty, but auto-halt must NOT fire in that case: the model's
/// intent was to run a command (the command is embedded in the leaked halt
/// reasoning). Parsing failure means we cannot extract that intent, and the
/// only safe recovery is parse-fault handling (evidence marker + belief decay
/// + re-strategize), not a synthesized halt that discards the intent.
fn proposal_was_malformed(raw: &str) -> bool {
    raw.contains("<|end|>")
        || raw.contains("<|start|>")
        || raw.contains("<rfl:")
        || raw.contains("to=execute_command")
        || raw.contains("to=halt")
        || raw.contains("to=read_file")
        || raw.contains("to=write_file")
        || raw.contains("assistant to=")
        || raw.contains("Assistant to=")
        || raw.contains("tool to=")
}

'''

src = src.replace(anchor, helper + anchor, 1)

# ---- 2. Guard both auto-halt sites.
# The condition is a fixed three-line pattern. Walk line-by-line so the
# guard lands only where it should.
lines = src.split('\n')
new_lines = []
patched = 0
i = 0
while i < len(lines):
    line = lines[i]
    stripped = line.strip()
    indent = line[:len(line) - len(line.lstrip())]
    # Match: `if status == ContractStatus::Satisfied` followed by the two
    # known continuation lines.
    if (stripped == 'if status == ContractStatus::Satisfied'
        and i + 2 < len(lines)
        and lines[i + 1].strip() == '&& unwritten_targets.is_empty()'
        and '!actions.iter().any(|a| matches!(a, AgentAction::Halt' in lines[i + 2]):
        new_lines.append(f'{indent}// Auto-halt is suppressed when the proposal failed'
                         f' to parse: parse-fault recovery must run so the'
                         f' model\'s intent (e.g. a leaked execute_command) is'
                         f' recovered on the next turn.')
        new_lines.append(f'{indent}if !proposal_was_malformed(&raw_proposal)')
        new_lines.append(f'{indent}    && status == ContractStatus::Satisfied')
        # The continuation lines follow; the next two will be `&&` clauses
        # starting at deeper indent. We consume them from `lines` and re-emit.
        new_lines.append(lines[i + 1])
        new_lines.append(lines[i + 2])
        patched += 1
        i += 3
        continue
    new_lines.append(line)
    i += 1

if patched == 0:
    sys.stderr.write("ERROR: no auto-halt sites matched\n")
    sys.exit(4)

sys.stderr.write(f"info: guarded {patched} auto-halt site(s)\n")
src = '\n'.join(new_lines)

# ---- 3. Append the regression test at the end of the file.
test_module = '''

#[cfg(test)]
mod proposal_sanity_tests {
    use super::proposal_was_malformed;

    #[test]
    fn detects_control_token_leak_from_failed_trace() {
        // Exact wire form from turn 23 of the marker_summary mission. The
        // model tried to emit halt-with-embedded-execute-command and the
        // parse rejected it as a control-token leak.
        let raw = r#"[{"type":"halt","reasoning":"The report still contains unverified line citations. I need numbered source output before rewriting marker_summary.md.<|end|><|start|>assistant to=execute_command<|channel|>analysis <|constrain|>xml<|message|><rfl:parameter name=\\"command\\">\\nnl -ba crates/runtime_core/src/evidence_paths.rs"}]"#;
        assert!(proposal_was_malformed(raw));
    }

    #[test]
    fn clean_proposal_is_not_malformed() {
        let clean = r#"[{"type":"halt","reasoning":"Deliverable written. No further action needed."}]"#;
        assert!(!proposal_was_malformed(clean));

        let write = r#"[{"type":"write_file","path":"out.md","content":"Report"}]"#;
        assert!(!proposal_was_malformed(write));
    }
}
'''

if src.rstrip().endswith('}'):
    src = src.rstrip() + '\n' + test_module
else:
    sys.stderr.write("ERROR: file does not end with closing brace\\n")
    sys.exit(5)

open(out_path, 'w', encoding='utf-8').write(src)
PY

echo
echo "===== diff (first 60 lines + last 40 lines) ====="
diff -u "$BACKUP" "$PATCHED" | head -80 || true
echo "..."
diff -u "$BACKUP" "$PATCHED" | tail -50 || true
echo "================================================="
echo

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

install -m 0644 "$PATCHED" "$SRC"
echo "installed patched strategize.rs"

rollback() {
  echo >&2
  echo "!! verification failed; rolling back" >&2
  cp "$BACKUP" "$SRC"
  exit 1
}
trap 'rollback' ERR

echo ">> cargo check -p nexus_runtime_core ..."
cargo check -p nexus_runtime_core || rollback

echo ">> cargo test -p nexus_runtime_core --lib proposal_sanity_tests ..."
cargo test -p nexus_runtime_core --lib proposal_sanity_tests || rollback

echo ">> cargo test -p nexus_runtime_core --lib 'cortical_loop::strategize' ..."
cargo test -p nexus_runtime_core --lib 'cortical_loop::strategize' || rollback

if [[ "$FULL" -eq 1 ]]; then
  echo ">> cargo test -p nexus_runtime_core --lib ..."
  cargo test -p nexus_runtime_core --lib || rollback
fi

trap - ERR
echo
echo "OK: patch applied and verified."
echo "Next: cargo fmt -p nexus_runtime_core"
