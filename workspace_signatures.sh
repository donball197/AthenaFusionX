#!/usr/bin/env bash
#
# workspace_signatures.sh — declaration index + structural bug-lead scan
#
# Fixes vs. the original awk one-liner:
#   1. keywords are whole-word anchored -> model*, modifier, modified_count,
#      modules, implementation no longer pollute the index
#   2. every hit prints "file:line: declaration" -> citable evidence ranges
#   3. deterministic file order; paths with spaces are safe
#   4. -a adds structural analysis: undeclared modules, declared-but-missing
#      modules, duplicate type names, stray `pub mod;`, empty/stub files
#
# Run from the workspace root (the directory containing crates/ apps/ bins/).
# GNU coreutils assumed (xargs -r).

set -euo pipefail

SIG_RE='^[[:space:]]*(pub[[:space:]]+)?(fn|struct|enum|trait|impl|mod)([^[:alnum:]_]|$)'
TYPE_RE='^[[:space:]]*(pub[[:space:]]+)?(struct|enum)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*'
MODDECL_RE='^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;'

OUT="active_workspace_signatures.txt"
REPORT="signature_analysis.txt"
ANALYZE=0
ROOTS=()

usage() {
  cat <<'EOF'
usage: workspace_signatures.sh [options] [ROOT ...]

  ROOT ...   scan roots (default: crates apps bins)
  -o FILE    signature index output  (default: active_workspace_signatures.txt)
  -a         also run structural analysis
  -R FILE    analysis report output  (default: signature_analysis.txt)
  -h         show this help

Examples:
  ./workspace_signatures.sh                       # index only
  ./workspace_signatures.sh -a                    # index + structural analysis
  ./workspace_signatures.sh -a -o idx.txt -R bugs.txt crates bins
EOF
}

while getopts ":o:aR:h" opt; do
  case "$opt" in
    o) OUT="$OPTARG" ;;
    a) ANALYZE=1 ;;
    R) REPORT="$OPTARG" ;;
    h) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
if (( $# )); then ROOTS=("$@"); else ROOTS=(crates apps bins); fi

for r in "${ROOTS[@]}"; do
  [[ -d "$r" ]] || { echo "error: scan root is not a directory: $r" >&2; exit 2; }
done

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# NUL-safe, deterministic file list
find "${ROOTS[@]}" -type f -name '*.rs' -print0 | sort -z > "$tmp/files0"

# ---------------------------------------------------------------------------
# Pass 1: signature index
# ---------------------------------------------------------------------------
cat > "$tmp/sig.awk" <<'AWK'
FNR == 1 {
    if (seen++) print ""
    print "========================================="
    print "FILE: " FILENAME
    print "========================================="
}
$0 ~ sig_re { printf "%d: %s\n", FNR, $0 }
AWK

xargs -0 -r awk -v sig_re="$SIG_RE" -f "$tmp/sig.awk" < "$tmp/files0" > "$OUT"

files_n=$(grep -c '^FILE:' "$OUT" || true)
syms_n=$(grep -cE '^[0-9]+: ' "$OUT" || true)
echo "index written: $OUT  ($files_n files, $syms_n signature lines)"

if (( ! ANALYZE )); then exit 0; fi

# ---------------------------------------------------------------------------
# Pass 2: structural analysis
# ---------------------------------------------------------------------------
{
  echo "workspace structural analysis — $(date -u +%FT%TZ)"
  echo "roots: ${ROOTS[*]}"

  # --- 0. noise audit: lines the OLD unanchored regex matched, new one does not
  echo
  echo "== 0. keyword-noise audit (lines dropped by the anchored regex) =="
  xargs -0 -r awk \
    -v old='^[[:space:]]*(pub[[:space:]]+)?(fn|struct|enum|trait|impl|mod)' \
    -v new="$SIG_RE" \
    '$0 ~ old && $0 !~ new { print FILENAME ":" FNR ": " $0 }' < "$tmp/files0"

  # --- 1. declared modules (name<TAB>declaring-file)
  xargs -0 -r awk -v re="$MODDECL_RE" '
    $0 ~ re {
      line = $0
      sub(/.*mod[[:space:]]+/, "", line)
      sub(/[[:space:]]*;.*/, "", line)
      print line "\t" FILENAME
    }' < "$tmp/files0" | sort -u > "$tmp/mods_declared"

  # --- 2. files no scanned source declares (likely dead / missing wiring)
  echo
  echo "== 2. module files with no 'mod <name>;' declaration found =="
  while IFS= read -r -d '' f; do
    base="$(basename "$f" .rs)"
    case "$base" in lib|main|build) continue ;; esac
    case "$f" in */bin/*|*/examples/*|*/tests/*) continue ;; esac
    name="$base"
    if [[ "$base" == "mod" ]]; then name="$(basename "$(dirname "$f")")"; fi
    if ! awk -F'\t' -v n="$name" '$1 == n { found = 1 } END { exit !found }' "$tmp/mods_declared"; then
      echo "  $f   (mod $name)"
    fi
  done < "$tmp/files0"

  # --- 3. declared modules with no matching file next to the declaration
  echo
  echo "== 3. 'mod <name>;' declarations with no matching file =="
  while IFS=$'\t' read -r name decl; do
    dir="$(dirname "$decl")"
    if [[ ! -f "$dir/$name.rs" && ! -f "$dir/$name/mod.rs" ]]; then
      echo "  $decl declares 'mod $name;' but $dir/$name.rs and $dir/$name/mod.rs do not exist"
    fi
  done < "$tmp/mods_declared"

  # --- 4. duplicate struct/enum names across files
  echo
  echo "== 4. struct/enum names defined in more than one file =="
  xargs -0 -r awk -v re="$TYPE_RE" '
    $0 ~ re {
      line = $0
      sub(/^[[:space:]]*(pub[[:space:]]+)?(struct|enum)[[:space:]]+/, "", line)
      sub(/[^A-Za-z0-9_].*$/, "", line)
      print line "\t" FILENAME ":" FNR
    }' < "$tmp/files0" | sort > "$tmp/types"
  awk -F'\t' '
    { c[$1]++; l[$1] = l[$1] (l[$1] ? ", " : "") $2 }
    END { for (n in c) if (c[n] > 1) print "  " n ": " l[n] }
  ' "$tmp/types" | sort

  # --- 5. stray `pub mod X;` outside lib/main/mod files
  echo
  echo "== 5. 'pub mod X;' in non-root files (suspect include/copy-paste) =="
  xargs -0 -r awk '
    /^[[:space:]]*pub[[:space:]]+mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;/ {
      n = FILENAME; sub(/.*\//, "", n)
      if (n != "lib.rs" && n != "main.rs" && n != "mod.rs")
        print "  " FILENAME ":" FNR ": " $0
    }' < "$tmp/files0"

  # --- 6. empty or near-empty files
  echo
  echo "== 6. files with fewer than 5 lines =="
  while IFS= read -r -d '' f; do
    n=$(wc -l < "$f")
    if (( n < 5 )); then echo "  $f ($n lines)"; fi
  done < "$tmp/files0"

  # --- 7. stub / placeholder markers
  echo
  echo "== 7. stub and placeholder markers =="
  xargs -0 -r grep -nE \
    'Stub|todo!|unimplemented!|panic!$$"not implemented|fn add\(left: u64, right: u64$$' \
    < "$tmp/files0" | sed 's/^/  /' || true

  # --- 8. follow-up tooling hint
  echo
  echo "== 8. follow-up (run separately) =="
  echo "  cargo clippy --all-targets -- -W dead_code   # reachability / dead code"
  echo "  cargo test --workspace                      # contract checks already encoded in tests"
} > "$REPORT"

echo "analysis written: $REPORT"
