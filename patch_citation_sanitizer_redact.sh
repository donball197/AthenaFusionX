#!/usr/bin/env bash
# Upgrade regex_lite_spans + ensure redact body (idempotent-ish).
# Usage: ./patch_citation_sanitizer_redact.sh [--apply]

set -euo pipefail
ROOT="$(pwd)"
CRITIC="$ROOT/crates/runtime_core/src/cortical_loop/critic.rs"
APPLY=0
for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    -h|--help) exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done
[[ -f "$CRITIC" ]] || { echo "ERROR: $CRITIC missing" >&2; exit 1; }

STAMP="$(date +%Y%m%d-%H%M%S)"
cp "$CRITIC" "$CRITIC.bak.$STAMP"
echo "backup: $CRITIC.bak.$STAMP"
PATCH="$(mktemp)"
trap 'rm -f "$PATCH"' EXIT

python3 - "$CRITIC" "$PATCH" <<'PY'
import sys
src_path, out_path = sys.argv[1], sys.argv[2]
src = open(src_path, encoding="utf-8").read()

def die(m):
    sys.stderr.write(m + "\n"); sys.exit(3)

def replace_fn(src, sig, new_body):
    start = src.find(sig)
    if start < 0:
        die(f"ERROR: {sig!r} not found")
    depth = 0
    end_rel = None
    rest = src[start:]
    for i, ch in enumerate(rest):
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                end_rel = i + 1
                break
    if end_rel is None:
        die(f"ERROR: no end for {sig}")
    return src[:start] + new_body + src[start + end_rel:]

# ── sanitizer: redact + guard ───────────────────────────────
san_sig = "fn sanitize_ungrounded_citation_veto(verdict: &str, evidence: &str) -> String {"
san_new = '''fn sanitize_ungrounded_citation_veto(verdict: &str, evidence: &str) -> String {
    let spans = regex_lite_spans(verdict);
    if spans.is_empty() {
        return verdict.to_string();
    }

    let mut out = verdict.to_string();
    let mut any_ungrounded = false;
    for span in &spans {
        if evidence.contains(span) {
            continue;
        }
        any_ungrounded = true;
        // Neutralize the token so the strategist cannot copy it as authority.
        out = out.replace(span, "unverified-span");
    }

    if any_ungrounded {
        out.push_str(
            "\\n[CITATION_GUARD] One or more prescribed line ranges in this veto do not \
appear in local evidence. Ignore unverified-span placeholders. Re-derive symbol spans \
from the existing READ_OK / READ_CHUNK_OK body and rewrite the report from that body only. \
Do not trust invented line ranges.",
        );
    }
    out
}'''
src = replace_fn(src, san_sig, san_new)

# ── span scanner: if "line" appears anywhere, take all N-M ──
span_sig = "fn regex_lite_spans(text: &str) -> Vec<String> {"
span_new = '''fn regex_lite_spans(text: &str) -> Vec<String> {
    // Candidate line-range tokens of the form N-M.
    // If the critique mentions "line" anywhere, every N-M is a candidate
    // (covers "not 95-105, 111-132, and 140-165" after an earlier "lines ...").
    // Otherwise require a local "line" window so version ranges in unrelated
    // vetoes are left alone.
    let line_context_global = text.to_ascii_lowercase().contains("line");
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < n && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let a = &text[start..i];
            if i < n && bytes[i] == b'-' {
                let j = i + 1;
                let mut k = j;
                while k < n && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > j {
                    let accept = if line_context_global {
                        true
                    } else {
                        let ctx_from = start.saturating_sub(12);
                        text[ctx_from..k].to_ascii_lowercase().contains("line")
                    };
                    if accept {
                        spans.push(format!("{}-{}", a, &text[j..k]));
                    }
                    i = k;
                    continue;
                }
            }
        }
        i += 1;
    }
    spans
}'''
src = replace_fn(src, span_sig, span_new)

open(out_path, "w", encoding="utf-8").write(src)
print("sanitizer + span scanner upgraded")
PY

echo
echo "===== diff ====="
diff -u "$CRITIC" "$PATCH" || true
echo "================"

if ! grep -q 'line_context_global' "$PATCH"; then
  echo "ERROR: span scanner upgrade missing" >&2
  exit 1
fi
if ! grep -q 'unverified-span' "$PATCH"; then
  echo "ERROR: redact logic missing" >&2
  exit 1
fi

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

install -m 0644 "$PATCH" "$CRITIC"
echo "patched"

rollback() {
  echo >&2; echo "!! verification failed; rolling back" >&2
  cp "$CRITIC.bak.$STAMP" "$CRITIC"; exit 1
}
trap 'rollback' ERR

cargo test -p nexus_runtime_core --lib 'citation_sanitizer_tests'
cargo test -p nexus_runtime_core --lib 'cortical_loop::critic'

trap - ERR
echo
echo "OK. Next: cargo fmt -p nexus_runtime_core"
