#!/usr/bin/env bash
# strategy_hardening.sh  —  surgical changes to algorithms/strategy/src/lib.rs
#
#   [1] patch_precedes_explicit_verification_command: actually set required_command
#   [2] patch_strategies_from_target: empty-function branch emits competing strategies
#   [3] new test pinning the multi-strategy contract for that branch
#
# DRY RUN by default: prints a unified diff, writes nothing.
#   ./strategy_hardening.sh                 # dry run
#   APPLY=1 ./strategy_hardening.sh         # write + backup + cargo test/check
#   ./strategy_hardening.sh [REPO_ROOT]     # default $HOME/AthenaFusionX
set -euo pipefail

[[ $# -le 1 ]] || { echo "usage: $0 [REPO_ROOT]" >&2; exit 2; }
command -v python3 >/dev/null || { echo "ERROR: python3 required." >&2; exit 2; }

REPO_ROOT="${1:-${REPO_ROOT:-$HOME/AthenaFusionX}}"
[[ -d "$REPO_ROOT" ]] || { echo "ERROR: not a directory: $REPO_ROOT" >&2; exit 2; }

export APPLY="${APPLY:-0}"

python3 - "$REPO_ROOT" <<'PYEOF'
import difflib, os, pathlib, shutil, sys, time

root = pathlib.Path(sys.argv[1])
APPLY = os.environ.get("APPLY", "0") == "1"
TS = time.strftime("%Y%m%d-%H%M%S")
REL = "algorithms/strategy/src/lib.rs"

p = root / REL
if not p.is_file():
    sys.exit("ERROR: target not found: %s" % p)

src = p.read_text(encoding="utf-8")
lines = src.split("\n")
changes = []


def die(msg):
    print("  [abort] %s" % msg)
    print("  [abort] nothing written.")
    sys.exit(1)


def region_of(fn_name, span=40):
    hits = [i for i, l in enumerate(lines) if "fn %s" % fn_name in l]
    if len(hits) != 1:
        die("anchor 'fn %s' matched %dx" % (fn_name, len(hits)))
    i0 = hits[0]
    end = None
    for j in range(i0, min(len(lines), i0 + span)):
        if lines[j].rstrip() == "    }":
            end = j
            break
    if end is None:
        die("could not find end of fn %s" % fn_name)
    return i0, end


# ---------------------------------------------------------------- [1]
print("=== [1/3] precedence test: set required_command ===")
i0, i1 = region_of("patch_precedes_explicit_verification_command")
region = lines[i0:i1 + 1]

if any("i.required_command" in l for l in region):
    print("  [skip] already applied.")
else:
    gi = [j for j, l in enumerate(region) if "let i = input(" in l]
    if len(gi) != 1:
        die("'let i = input(' matched %dx in region" % len(gi))
    close = [j for j, l in enumerate(region) if l.rstrip() == "        );"]
    if len(close) != 1:
        die("input-call terminator '        );' matched %dx in region" % len(close))

    region[gi[0]] = region[gi[0]].replace("let i = input(", "let mut i = input(")
    region.insert(close[0] + 1,
                  '        i.required_command = Some("cargo test -p math_utils".into());')
    lines[i0:i1 + 1] = region
    changes.append("[1] required_command set in patch_precedes_explicit_verification_command")
    print("  [ok]   'let i' -> 'let mut i'; required_command injected after input(...).")


# ---------------------------------------------------------------- [2]
print("\n=== [2/3] patch_strategies_from_target: competing strategies ===")
anchor = 'vec![format!("patch_function:{}", target)]'
gi = [i for i, l in enumerate(lines) if l.strip() == anchor]

if len(gi) == 0:
    if 'format!("read_file:{}", path)' in "\n".join(lines):
        print("  [skip] already applied.")
    else:
        die("anchor '%s' absent and no evidence of prior fix" % anchor)
elif len(gi) == 1:
    indent = " " * (len(lines[gi[0]]) - len(lines[gi[0]].lstrip()))
    lines[gi[0]:gi[0] + 1] = [
        indent + "vec![",
        indent + '    format!("patch_function:{}", target),',
        indent + '    format!("read_file:{}", path),',
        indent + '    format!("replace_lines:{}", path),',
        indent + "]",
    ]
    changes.append("[2] empty-function branch emits 3 competing strategies")
    print("  [ok]   single-hypothesis vec replaced by 3-strategy block.")
else:
    die("anchor matched %dx; nothing written." % len(gi))


# ---------------------------------------------------------------- [3]
print("\n=== [3/3] new test: patch_target_without_function_still_emits_competing_strategies ===")
NEW_TEST = '''
    #[test]
    fn patch_target_without_function_still_emits_competing_strategies() {
        // PATCH_FUNCTION_OK form with an empty symbol half ("path:").
        // The primary hypothesis keeps the raw target verbatim; the two
        // competing strategies guarantee the graph is never locked into a
        // single hypothesis (contract of `patch_strategies`).
        let out =
            DirectiveStrategyCompiler::patch_strategies_from_target("apps/math_utils/src/lib.rs:");
        assert_eq!(
            out,
            vec![
                "patch_function:apps/math_utils/src/lib.rs:",
                "read_file:apps/math_utils/src/lib.rs",
                "replace_lines:apps/math_utils/src/lib.rs",
            ]
        );
    }'''

if "patch_target_without_function_still_emits_competing_strategies" in "\n".join(lines):
    print("  [skip] already applied.")
else:
    anchor = 'assert!(DirectiveStrategyCompiler::compile(&i).is_none());'
    gi = [i for i, l in enumerate(lines) if l.strip() == anchor]
    if len(gi) != 1:
        die("test anchor matched %dx; nothing written." % len(gi))
    end = None
    for j in range(gi[0], min(len(lines), gi[0] + 10)):
        if lines[j].rstrip() == "    }":
            end = j
            break
    if end is None:
        die("could not find closing brace of no_deterministic_directive_returns_none")
    lines.insert(end + 1, NEW_TEST)
    changes.append("[3] new test patch_target_without_function_still_emits_competing_strategies")
    print("  [ok]   test inserted after no_deterministic_directive_returns_none.")


# ---------------------------------------------------------------- report
new_src = "\n".join(lines)
if new_src == src:
    print("\n=== No changes pending. File already in target state. ===")
    sys.exit(0)

diff = "\n".join(difflib.unified_diff(
    src.split("\n"), new_src.split("\n"),
    fromfile="a/" + REL, tofile="b/" + REL, lineterm=""))
print("\n=== Diff ===")
print(diff)

if not APPLY:
    print("\n=== DRY RUN: nothing written (%d change block(s) pending). ===" % len(changes))
    print("Re-run with APPLY=1 to apply.")
    sys.exit(0)

b = p.with_name(p.name + ".bak.strategyfix." + TS)
shutil.copy2(p, b)
p.write_text(new_src, encoding="utf-8")
print("\n[write] %s  (backup: %s)" % (p, b.name))
for c in changes:
    print("  applied: %s" % c)
PYEOF

echo ""
echo "=== Verify ==="
grep -n "let mut i = input(\|i.required_command = Some" "$REPO_ROOT/algorithms/strategy/src/lib.rs" || true
grep -n 'read_file:{\|replace_lines:{}' "$REPO_ROOT/algorithms/strategy/src/lib.rs" || true
grep -n "patch_target_without_function_still_emits_competing_strategies" \
  "$REPO_ROOT/algorithms/strategy/src/lib.rs" || true

if [[ "$APPLY" != "1" ]]; then
    echo ""
    echo "DRY RUN complete. Re-run with APPLY=1 to write, then:"
    echo "  cargo test -p nexus_strategy_algorithms"
    exit 0
fi

echo ""
echo "=== Tests ==="
( cd "$REPO_ROOT" && cargo test -p nexus_strategy_algorithms )

echo ""
echo "=== Compile check (runtime_core untouched, must stay clean) ==="
( cd "$REPO_ROOT" && cargo check -p nexus_runtime_core )

echo ""
echo "=== Done ==="
