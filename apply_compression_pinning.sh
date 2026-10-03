#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
elif [[ -n "${1:-}" ]]; then
    echo "[ERROR] unknown argument: $1" >&2
    exit 2
fi

WORKSPACE_ROOT="$(realpath -m "${WORKSPACE_ROOT:-$(pwd)}")"
echo "=== compression pinning v2 | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

EP="crates/runtime_core/src/epistemic_engine.rs"
if [[ ! -f "$WORKSPACE_ROOT/$EP" ]]; then
    echo "[PREFLIGHT FAILED] missing $EP" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
EP_REL="$EP" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
EP   = os.environ["EP_REL"]

path = root / EP
s0 = path.read_text(encoding="utf-8")
s = s0

COMPRESS = r'''    /// Prompt-window `evidence` while keeping physically read source whole.
    ///
    /// RULE 6 and the verifier's quote clause require the judging model to
    /// SEE the source a halt's claims are about; naive head/tail windows
    /// elide the middle of a large [READ_OK] ... FULL FILE CONTENT block --
    /// exactly where the interesting lines live -- so grounded claims fail
    /// for lack of visible quotes. This windows only the non-read material;
    /// every read-record region survives verbatim and in order.
    /// Region boundaries use a POSITIVE list of known evidence markers: an
    /// unknown bracketed line (e.g. a source line starting with `[ERROR]`)
    /// does NOT end the pinned region -- when in doubt we over-pin, never
    /// silently drop source. Oversized regions are capped with a VISIBLE
    /// marker, never silently, because the critic may run on a small-context
    /// tier. The strategist keeps its own strict 2500-char edge budget on
    /// purpose (4K-context persona); only judgment layers pin reads.
    pub(crate) fn compress_preserving_reads(evidence: &str, head: usize, tail: usize) -> String {
        const MAX_PINNED_REGION: usize = 12_000;

        let starts_read_region = |line: &str| {
            let t = line.trim_start();
            t.starts_with("[READ_OK]") || t.starts_with("[READ_CHUNK_OK]")
        };
        let ends_region = |line: &str| {
            const ENDERS: &[&str] = &[
                "[READ_OK]", "[READ_CHUNK_OK]", "[WRITE_OK]", "[CMD_OK]", "[SEARCH_CODE_OK]",
                "[SEARCH_MEMORY_OK]", "[PATCH_FUNCTION_OK]", "[PATCH_IDEMPOTENT_OK]",
                "[DIFF_PATCH_OK]", "[REPLACE_LINES_OK]", "[RUN_TESTS_OK]", "[COMPILE_OK]",
                "[PROPOSITION]", "[HALT", "[CRITIC_VETO]", "[CRITIC_FACT_OVERRIDE]",
                "[HALT_REJECTED]", "[PHYSICAL SESSION EVIDENCE]",
            ];
            let t = line.trim_start();
            ENDERS.iter().any(|m| t.starts_with(m)) && !starts_read_region(t)
        };

        enum Seg {
            Pinned(String),
            Plain(String),
        }

        let lines: Vec<&str> = evidence.lines().collect();
        let mut segs: Vec<Seg> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if starts_read_region(lines[i]) {
                let mut region = String::new();
                while i < lines.len() {
                    let cur = lines[i];
                    if !region.is_empty() && ends_region(cur) {
                        break;
                    }
                    region.push_str(cur);
                    region.push('\n');
                    i += 1;
                }
                let pinned = if region.chars().count() > MAX_PINNED_REGION {
                    let kept: String = region.chars().take(MAX_PINNED_REGION).collect();
                    format!(
                        "{}\n[... READ BODY CAPPED AT {} CHARS IN JUDGE WINDOW ...]",
                        kept, MAX_PINNED_REGION
                    )
                } else {
                    region.trim_end_matches('\n').to_string()
                };
                segs.push(Seg::Pinned(pinned));
            } else {
                segs.push(Seg::Plain(lines[i].to_string()));
                i += 1;
            }
        }

        let plain_total: usize = segs
            .iter()
            .map(|s| match s {
                Seg::Plain(l) => l.len() + 1,
                Seg::Pinned(_) => 0,
            })
            .sum();
        let render = |s: &Seg| match s {
            Seg::Plain(l) => l.clone(),
            Seg::Pinned(r) => r.clone(),
        };
        if plain_total <= head + tail {
            return segs.iter().map(render).collect::<Vec<_>>().join("\n");
        }

        let n = segs.len();
        let mut head_end = 0usize;
        let mut used = 0usize;
        while head_end < n && used < head {
            if let Seg::Plain(l) = &segs[head_end] {
                used += l.len() + 1;
            }
            head_end += 1;
        }
        let mut tail_start = n;
        used = 0;
        while tail_start > head_end && used < tail {
            if let Seg::Plain(l) = &segs[tail_start - 1] {
                used += l.len() + 1;
            }
            tail_start -= 1;
        }

        let mut out: Vec<String> = Vec::new();
        for s in &segs[..head_end] {
            out.push(render(s));
        }
        let middle = &segs[head_end..tail_start];
        if middle.iter().any(|s| matches!(s, Seg::Plain(_))) {
            out.push("[... MIDDLE EVIDENCE COMPRESSED ...]".to_string());
        }
        for s in middle {
            if matches!(s, Seg::Pinned(_)) {
                out.push(render(s));
            }
        }
        for s in &segs[tail_start..] {
            out.push(render(s));
        }
        out.join("\n")
    }

'''

CRITIC_FILTER = r'''    /// Evidence history as the CRITIC may see it. Prior critic verdicts and
    /// halt rejections are feedback for the STRATEGIST, not evidence for the
    /// critic: keeping them let the critic restate its previous verdict as a
    /// fresh one (observed echo: a veto about "no defect found" issued
    /// against a proposal that DID report a defect).
    pub(crate) fn critic_visible_history(evidence_history: &str) -> String {
        evidence_history
            .split("\n\n")
            .filter(|c| {
                !c.trim().is_empty()
                    && !c.contains("[CRITIC_VETO]")
                    && !c.contains("[HALT_REJECTED]")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

'''

CRITIC_SITE = '''        let filtered_history = Self::critic_visible_history(evidence_history);
        // Judgment-layer window: recent material plus every physically read
        // source region, so RULE 6 can pair claims with quotes.
        let grounded_evidence = Self::compress_preserving_reads(&filtered_history, 1500, 2500);'''

EVAL_OLD = r'''        let truncated_evidence: String = if evidence.chars().count() <= 2500 {
            evidence.to_string()
        } else {
            let start: String = evidence.chars().take(1000).collect();
            let end: String =
                evidence.chars().skip(evidence.chars().count().saturating_sub(1500)).collect();
            format!("{}...\n\n[... MIDDLE EVIDENCE COMPRESSED ...]\n\n{}", start, end)
        };'''
EVAL_NEW = r'''        let truncated_evidence: String = Self::compress_preserving_reads(evidence, 1000, 1500);'''

VER_OLD = r'''        let truncated_evidence: String = if evidence.chars().count() <= 14000 {
            evidence.to_string()
        } else {
            let start: String = evidence.chars().take(3000).collect();
            let end: String =
                evidence.chars().skip(evidence.chars().count().saturating_sub(11000)).collect();
            format!("{}...\n\n[... MIDDLE EVIDENCE COMPRESSED ...]\n\n{}", start, end)
        };'''
VER_NEW = r'''        let truncated_evidence: String = Self::compress_preserving_reads(evidence, 3000, 11000);'''

TEST_ANCHOR = "    #[tokio::test]\n    async fn compound_read_plus_search_code_pushes_search_first() {"

TESTS = {
"compress_preserving_reads_keeps_whole_file_content": r'''    #[test]
    fn compress_preserving_reads_keeps_whole_file_content() {
        let body: String = (0..200)
            .map(|i| format!("fn line_{}() {{}}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let read = format!("[READ_OK] x.rs\nFULL FILE CONTENT:\n{}\n", body);
        let noise: String = (0..120).map(|i| format!("noise {}", i)).collect::<Vec<_>>().join("\n");
        let evidence = format!("line_a\nline_b\n{}\n[PROPOSITION] end-of-read\n{}\ntail_marker", read, noise);

        let out = EpistemicEngine::compress_preserving_reads(&evidence, 64, 64);
        assert!(out.contains("FULL FILE CONTENT:"), "read record must survive");
        assert!(out.contains("fn line_199() {}"), "end of the file body must survive");
        assert!(out.contains("line_a"), "head must survive");
        assert!(out.contains("tail_marker"), "tail must survive");
        assert!(out.contains("[... MIDDLE EVIDENCE COMPRESSED ...]"), "elision must be marked");
        assert!(!out.contains("noise 100"), "middle non-read noise must be elided");
    }

''',
"compress_preserving_reads_keeps_short_evidence_verbatim": r'''    #[test]
    fn compress_preserving_reads_keeps_short_evidence_verbatim() {
        let evidence = "[READ_OK] x.rs\nFULL FILE CONTENT:\nfn main() {}";
        let out = EpistemicEngine::compress_preserving_reads(evidence, 64, 64);
        assert_eq!(out, evidence);
    }

''',
"compress_preserving_reads_caps_oversized_reads_visibly": r'''    #[test]
    fn compress_preserving_reads_caps_oversized_reads_visibly() {
        let body: String = (0..2000)
            .map(|i| format!("fn line_{}() {{}}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let read = format!("[READ_OK] big.rs\nFULL FILE CONTENT:\n{}\n", body);
        let out = EpistemicEngine::compress_preserving_reads(&read, 64, 64);
        assert!(
            out.contains("[... READ BODY CAPPED AT 12000 CHARS IN JUDGE WINDOW ...]"),
            "oversized reads must be capped with a visible marker"
        );
        assert!(out.contains("fn line_0() {}"), "the head of the read must survive");
    }

''',
"bracketed_source_lines_do_not_end_a_pinned_read": r'''    #[test]
    fn bracketed_source_lines_do_not_end_a_pinned_read() {
        let read = "[READ_OK] s.rs\nFULL FILE CONTENT:\n[ERROR] marker-shaped source line\nfn after() {}\n";
        let out = EpistemicEngine::compress_preserving_reads(read, 8, 8);
        assert!(
            out.contains("fn after() {}"),
            "unknown bracketed lines must not terminate the pinned read"
        );
    }

''',
"critic_visible_history_drops_echo_chunks_and_keeps_reads": r'''    #[test]
    fn critic_visible_history_drops_echo_chunks_and_keeps_reads() {
        let read = "[READ_OK] x.rs\nFULL FILE CONTENT:\nfn target() {}\n";
        let history = format!(
            "{}\n\n[CRITIC_VETO]\nold verdict text\n\n[HALT_REJECTED]\nold rejection text\n\ntail_line",
            read
        );
        let visible = EpistemicEngine::critic_visible_history(&history);
        assert!(!visible.contains("old verdict text"), "echo veto must be filtered");
        assert!(!visible.contains("old rejection text"), "old halt rejections must be filtered");
        assert!(visible.contains("fn target() {}"), "physically read source must remain visible");
        let windowed = EpistemicEngine::compress_preserving_reads(&visible, 64, 64);
        assert!(windowed.contains("fn target() {}"), "judge window must keep the read whole");
    }

''',
}

# ---------------- steps -------------------------------------------------
def step(label, applied):
    print(f"[STATUS] {label:52s} : {applied}")

# step 1: helpers
if "fn compress_preserving_reads" in s:
    if "MAX_PINNED_REGION" not in s:
        print("[STATUS] compress_preserving_reads helper               : FAIL (v1 helper shape: uncapped/heuristic)")
        print("  -> remove the v1 helper first (it aborts pinning semantics), then re-run")
        sys.exit(1)
    step("compress_preserving_reads helper", "SKIP (v2 present)")
elif s.count("    pub async fn generate_hypotheses(") == 1:
    s = s.replace("    pub async fn generate_hypotheses(", COMPRESS + "    pub async fn generate_hypotheses(", 1)
    step("compress_preserving_reads helper", "APPLY")
else:
    step("compress_preserving_reads helper", "FAIL (anchor not unique)")
    sys.exit(1)

if "pub(crate) fn critic_visible_history" in s:
    step("critic_visible_history helper", "SKIP (already present)")
elif s.count("    pub async fn generate_hypotheses(") == 1:
    s = s.replace("    pub async fn generate_hypotheses(", CRITIC_FILTER + "    pub async fn generate_hypotheses(", 1)
    step("critic_visible_history helper", "APPLY")
else:
    step("critic_visible_history helper", "FAIL (anchor not unique)")
    sys.exit(1)

# step 2: critic site (three known shapes)
if "Self::critic_visible_history(evidence_history)" in s:
    step("critic site: echo filter + pinned window", "SKIP (v2 shape present)")
elif s.count("        const K: usize = 6;") == 1:
    start = s.index("        const K: usize = 6;")
    end = s.index("\n        };", start) + len("\n        };")
    span = s[start:end]
    if "let grounded_evidence" not in span or "[... MIDDLE EVIDENCE COMPRESSED ...]" not in span:
        step("critic site: echo filter + pinned window", "FAIL (span sanity)")
        sys.exit(1)
    s = s[:start] + CRITIC_SITE + s[end:]
    step("critic site: echo filter + pinned window", "APPLY (pre-compression span replaced)")
elif "let filtered_history: String = evidence_history" in s:
    bstart = s.index("        let filtered_history: String = evidence_history")
    bend = s.index('.join("\n\n");', bstart) + len('.join("\n\n");')
    s = s[:bstart] + "        let filtered_history = Self::critic_visible_history(evidence_history);" + s[bend:]
    step("critic site: echo filter + pinned window", "APPLY (v1 shape refactored)")
else:
    step("critic site: echo filter + pinned window", "FAIL (unknown shape)")
    sys.exit(1)

# step 3: evaluator+reflect, verifier
n = s.count(EVAL_OLD)
if "compress_preserving_reads(evidence, 1000, 1500)" in s and n == 0:
    step("evaluator + reflect windows", "SKIP (already present)")
elif n in (1, 2):
    s = s.replace(EVAL_OLD, EVAL_NEW, 2)
    step("evaluator + reflect windows", f"APPLY ({n} site(s))")
else:
    step("evaluator + reflect windows", f"FAIL (count={n})")
    sys.exit(1)

if "compress_preserving_reads(evidence, 3000, 11000)" in s:
    step("verifier window", "SKIP (already present)")
elif s.count(VER_OLD) == 1:
    s = s.replace(VER_OLD, VER_NEW, 1)
    step("verifier window", "APPLY")
else:
    step("verifier window", f"FAIL (count={s.count(VER_OLD)})")
    sys.exit(1)

# step 4: behavioral tests
for name, code in TESTS.items():
    if f"fn {name}(" in s:
        step(f"test {name}", "SKIP (already present)")
        continue
    if s.count(TEST_ANCHOR) != 1:
        step(f"test {name}", "FAIL (anchor not unique)")
        sys.exit(1)
    s = s.replace(TEST_ANCHOR, code + TEST_ANCHOR, 1)
    step(f"test {name}", "APPLY")

# step 5: remove vacuous v1 guard if present
marker = "    fn judgment_layers_pin_read_evidence() {"
if marker in s:
    mstart = s.index(marker)
    mstart = s.rindex("    #[test]", 0, mstart)
    extra = 0
    for ln in reversed(s[:mstart].splitlines(keepends=True)):
        if ln.strip().startswith("//") and extra < 400:
            extra += len(ln)
        else:
            break
    mstart -= extra
    mend = s.index("\n    }\n", mstart) + len("\n    }\n")
    s = s[:mstart] + s[mend:]
    step("remove vacuous judgment_layers_pin_read_evidence", "APPLY")
else:
    step("remove vacuous judgment_layers_pin_read_evidence", "SKIP (absent)")

# ---------------- post checks (vacuity-audited) -------------------------
for probe in (
    "pub(crate) fn compress_preserving_reads",
    "pub(crate) fn critic_visible_history",
    "&filtered_history, 1500, 2500)",
    "MAX_PINNED_REGION",
    "bounded_evidence",
):
    assert probe in s, f"POST-PATCH FAILED: missing {probe!r}"
assert s.count("compress_preserving_reads(evidence, 1000, 1500)") == 2, "eval/reflect sites must both be pinned"
assert s.count("compress_preserving_reads(evidence, 3000, 11000)") == 1, "verifier site must be pinned"
assert "judgment_layers_pin_read_evidence" not in s
assert s.count("fn compress_preserving_reads_keeps_whole_file_content") == 1
print("[POST-PATCH OK] probes vacuity-audited (test-name probes verify insertion only; behavior is covered by cargo test)")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
print("".join(difflib.unified_diff(s0.splitlines(keepends=True), s.splitlines(keepends=True),
                                  fromfile=f"a/{EP}", tofile=f"b/{EP}")))

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

path.write_text(s, encoding="utf-8")
print(f"[PATCH_OK] {EP}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review status lines and diff above."
else
    echo "[OK] compression pinning applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib epistemic_engine"
    echo ">> Full suite:"
    echo "     cargo test -p nexus_runtime_core"
fi
