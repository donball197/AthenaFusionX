#!/usr/bin/env bash
# patch_r5_supervise.sh — wire supervise_worker into background spawn sites
#
# What it does
#  1. Backs up bins/ide_backend/src/main.rs
#  2. Rewrites each of the five background spawn sites:
#         X::spawn(args...);
#     to
#         { let <v> = <v>.clone(); ...
#           tokio::spawn(supervise_worker("X", move || {
#               let handle = X::spawn(args...);
#               async move { handle.await.map_err(|e| anyhow::anyhow!("{e}")) }
#           })); }
#  3. Runs `cargo check` and the regression guard test.
#     Rolls back on any failure.
#
# Assumes
#  - Each X::spawn returns tokio::task::JoinHandle<()>.
#  - Each spawn arg is of the form <ident>.clone() (true for all five sites
#    as of the 2026-11-11 audit).
#  - supervise_worker has no Send bounds. If cargo check fails with a Send
#    error, add `+ Send` to F and Fut in supervise_worker's where-clause,
#    then re-run.
#
# Usage
#   ./patch_r5_supervise.sh                     # dry-run (prints diff)
#   ./patch_r5_supervise.sh --apply             # apply, check, rollback on failure
#   ./patch_r5_supervise.sh --apply --no-test   # skip the regression test
#   ./patch_r5_supervise.sh --help

set -euo pipefail

ROOT="$(pwd)"
MAIN="$ROOT/bins/ide_backend/src/main.rs"
APPLY=0
RUN_TEST=1

usage() { sed -n '2,30p' "$0"; }

for arg in "$@"; do
  case "$arg" in
    --apply)      APPLY=1 ;;
    --no-test)    RUN_TEST=0 ;;
    -h|--help)    usage; exit 0 ;;
    *)            echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

[[ -f "$MAIN" ]] || { echo "ERROR: $MAIN not found (run from repo root)" >&2; exit 1; }

# ---------- preconditions ----------
grep -q 'fn supervise_worker' "$MAIN" \
  || { echo "ERROR: fn supervise_worker not found in $MAIN" >&2; exit 1; }

MODULES=(dream_cycle self_healing_engine telemetry_bridge thermal_governor heartbeat)
for m in "${MODULES[@]}"; do
  if ! grep -q "${m}::spawn(" "$MAIN"; then
    echo "ERROR: ${m}::spawn( not found in $MAIN" >&2
    exit 1
  fi
done

# ---------- backup ----------
STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$MAIN.bak.$STAMP"
cp "$MAIN" "$BACKUP"
echo "backup: $BACKUP"

# ---------- build patched file ----------
PATCHED="$(mktemp)"
trap 'rm -f "$PATCHED"' EXIT

python3 - "$MAIN" "$PATCHED" "${MODULES[@]}" <<'PY'
import re, sys

src_path, out_path, *modules = sys.argv[1:]
src = open(src_path).read()

for name in modules:
    # Match the whole call statement: leading indent, X::spawn(...), semicolon.
    # `[^;]*` is greedy and safe because spawn args contain no ';'.
    pat = re.compile(
        r'(?m)^([ \t]*)' + re.escape(name) + r'::spawn\s*\(([^;]*)\)\s*;',
        re.S,
    )

    def repl(match, module=name):
        indent = match.group(1)
        argtext = match.group(2).strip()

        # Split top-level args by comma (respect nesting depth).
        args, depth, cur = [], 0, ""
        for ch in argtext:
            if ch == '(':
                depth += 1; cur += ch
            elif ch == ')':
                depth -= 1; cur += ch
            elif ch == ',' and depth == 0:
                args.append(cur.strip()); cur = ""
            else:
                cur += ch
        if cur.strip():
            args.append(cur.strip())

        # Every arg must be `<ident>.clone()` for this auto-patch to be safe.
        vars_in_order = []
        for a in args:
            mm = re.fullmatch(r'(\w+)\.clone\(\)', a)
            if not mm:
                sys.stderr.write(
                    f"ERROR: cannot auto-patch {module}::spawn arg {a!r} "
                    f"(expected <ident>.clone()). Patch that site manually.\n"
                )
                sys.exit(3)
            vars_in_order.append(mm.group(1))

        # Dedup preserving order.
        seen, uniq = set(), []
        for v in vars_in_order:
            if v not in seen:
                seen.add(v); uniq.append(v)

        pre = "".join(f"{indent}    let {v} = {v}.clone();\n" for v in uniq)
        inner_args = ", ".join(f"{v}.clone()" for v in vars_in_order)

        block = (
            f"{indent}{{\n"
            f"{pre}"
            f"{indent}    tokio::spawn(supervise_worker(\"{module}\", move || {{\n"
            f"{indent}        let handle = {module}::spawn({inner_args});\n"
            f"{indent}        async move {{ handle.await.map_err(|e| anyhow::anyhow!(\"{{e}}\")) }}\n"
            f"{indent}    }}));\n"
            f"{indent}}}"
        )
        return block

    new_src, n = pat.subn(repl, src)
    if n == 0:
        sys.stderr.write(f"ERROR: failed to locate {name}::spawn(...);\n")
        sys.exit(4)
    if n > 1:
        sys.stderr.write(f"WARN: {name}::spawn(...) matched {n} times; all rewritten\n")
    src = new_src

open(out_path, "w").write(src)
PY

# ---------- show diff ----------
echo
echo "===== diff ($BACKUP → patched) ====="
diff -u "$BACKUP" "$PATCHED" || true
echo "===================================="
echo

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

install -m 0644 "$PATCHED" "$MAIN"
echo "installed patched main.rs"

# ---------- verify with rollback ----------
rollback() {
  echo >&2
  echo "!! verification failed; rolling back" >&2
  cp "$BACKUP" "$MAIN"
  echo "restored $MAIN from $BACKUP" >&2
  exit 1
}
trap 'rollback' ERR

echo ">> cargo check -p ide_backend ..."
cargo check -p ide_backend || rollback

if [[ "$RUN_TEST" -eq 1 ]]; then
  echo ">> cargo test -p ide_backend regression_guards ..."
  cargo test -p ide_backend regression_guards || rollback
fi

trap - ERR
echo
echo "OK: patch applied and verified."
echo "Next: run 'cargo fmt -p ide_backend' to normalize formatting."
