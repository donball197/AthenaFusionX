#!/usr/bin/env bash
# Rust signature indexer.
# Output format:
#   =========================================
#   FILE: path
#   =========================================
#   <line>: <signature line>
#
# Handles: async/unsafe/const/default/extern fn, pub(crate|super|self|in ..),
# trait, type, const, static, impl<...>, mod, macro_rules!, union.
# Skips: target/, .git/, node_modules/, vendor/, .patch_backups/,
#        scratch/, and any cargo-fuzz target under fuzz/fuzz_targets/.
set -euo pipefail

ROOT="${1:-.}"
OUT="${2:-active_workspace_signatures.txt}"
cd "$ROOT"

VIS='(pub(\((crate|super|self|in [A-Za-z0-9_:]+)\))?[[:space:]]+)?'
QUAL='((default|const|async|unsafe|extern([[:space:]]+"[^"]*")?)[[:space:]]+)*'
PAT="^[[:space:]]*${VIS}(${QUAL}fn[[:space:]]|struct[[:space:]]|enum[[:space:]]|union[[:space:]]|trait[[:space:]]|unsafe[[:space:]]+trait[[:space:]]|type[[:space:]]|const[[:space:]]+[A-Za-z_]|static[[:space:]]|impl[[:space:]<]|mod[[:space:]])|^[[:space:]]*macro_rules!"

: > "$OUT"
files=0
sigs=0
empty=()

while IFS= read -r f; do
  files=$((files + 1))
  body=$(grep -nE "$PAT" "$f" | sed -E 's/[[:space:]]*\{[[:space:]]*$/ {/' || true)

  {
    echo "========================================="
    echo "FILE: ${f#./}"
    echo "========================================="
    [ -n "$body" ] && printf '%s\n' "$body" | sed -E 's/^([0-9]+):/\1: /'
    echo
  } >> "$OUT"

  if [ -z "$body" ]; then
    # Skip warning for cargo-fuzz targets (intentionally pure-macro)
    if [ "$(wc -l < "$f")" -gt 5 ] && [[ "$f" != *"/fuzz/fuzz_targets/"* ]]; then
      empty+=("${f#./}")
    fi
  else
    sigs=$((sigs + $(printf '%s\n' "$body" | wc -l)))
  fi
done < <(
  find . -type f -name '*.rs' \
    -not -path '*/target/*' \
    -not -path '*/.git/*' \
    -not -path '*/node_modules/*' \
    -not -path '*/vendor/*' \
    -not -path '*/.patch_backups/*' \
    -not -path '*/scratch/*' \
    | LC_ALL=C sort
)

echo "index written: $OUT  ($files files, $sigs signature lines)"
if [ ${#empty[@]} -gt 0 ]; then
  echo "WARN: ${#empty[@]} files >5 lines but zero signatures (check manually):"
  printf '  %s\n' "${empty[@]}"
fi
