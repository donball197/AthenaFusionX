#!/usr/bin/env bash
cd "$(dirname "$0")" || exit 1
O=audit_out; mkdir -p "$O"; E="$O/evidence.md"
DIRS=$(ls -d bins apps crates 2>/dev/null)
EX=(--include='*.rs' --exclude-dir=target --exclude-dir=.git --exclude='*.bak*' --exclude='*.pre_fix*')

cargo check --workspace --message-format=short >"$O/cargo_full.txt" 2>&1
echo "cargo exit: $?" >"$O/cargo_exit.txt"

{
echo "# Audit Evidence ($(date -u +%FT%TZ))"
echo; echo "## 1. Cargo"; cat "$O/cargo_exit.txt"
grep -E 'generated [0-9]+ warning' "$O/cargo_full.txt"
echo; echo "### Warnings by file"
grep ': warning:' "$O/cargo_full.txt" | sed -E 's/^([^:]+):.*/\1/' | sort | uniq -c | sort -rn
echo; echo "### All warnings"
grep ': warning:' "$O/cargo_full.txt"

echo; echo "## 2. Backup/orphan files"
find . \( -path ./target -o -path ./.git \) -prune -o \
  \( -name '*.bak*' -o -name '*.pre_fix*' -o -name '*.orig' -o -name '*~' \) -print

echo; echo "## 3. todo!/unimplemented! (call syntax, excluding .bak)"
grep -rnE "${EX[@]}" '\b(todo|unimplemented)!\s*[(\[]' $DIRS | tee "$O/stubs_all.txt" | wc -l
echo "### Likely real (no string literal on the line, not a comment)"
grep -vE '"|^\S+:[0-9]+:\s*//' "$O/stubs_all.txt"

echo; echo "## 4. Debt patterns"
for p in 'unwrap_or_default\(\)' 'process::exit' '\.unwrap\(\)' '\.expect\(' 'unsafe\s*\{' 'TODO|FIXME|HACK|XXX' '&[a-z_]+\[\.\.[0-9]+\]'; do
  echo "### $p"
  grep -rnE "${EX[@]}" "$p" $DIRS | grep -v '/tests/' | head -40
done

echo; echo "## 5. Duplicate fn names across files (heuristic)"
grep -rnoE "${EX[@]}" 'fn [a-z_0-9]+' $DIRS \
 | awk -F: '{print $1" "$3}' | sort -u | awk '{c[$2" "$3]++; f[$2" "$3]=f[$2" "$3]" "$1} END{for(k in c) if(c[k]>1) print c[k], k, f[k]}' \
 | grep -vE ' fn (new|fmt|default|from|main|init|spawn|run|drop|clone|eq|try_from|test_.*) ' | sort -rn | head -40
} >"$E" 2>&1
echo "wrote $E"; wc -l "$E"
