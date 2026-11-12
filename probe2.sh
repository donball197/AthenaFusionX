#!/usr/bin/env bash
# probe2.sh — READ-ONLY. Dumps the four function bodies we need to patch.
set -euo pipefail
ROOT="${1:-$(pwd)}"; cd "$ROOT"
OUT="${TMPDIR:-/tmp}/cortex_probe2_$(date +%Y%m%d_%H%M%S).log"
: > "$OUT"
say() { echo "$@" | tee -a "$OUT"; }

say "=============================================="
say ">> CORTEX PROBE 2  ($ROOT)"
say "=============================================="

# ---- [A] CorticalLoop::write_phase_ready body + test that pins it ----
say
say "== [A] write_phase_ready fn (cortical_loop.rs ~640-740) =="
sed -n '630,760p' crates/runtime_core/src/cortical_loop.rs | tee -a "$OUT"

say
say "== [A2] write_phase_ready regression test (loop_tests.rs ~140-180) =="
sed -n '130,185p' crates/runtime_core/src/cortical/loop_tests.rs | tee -a "$OUT"

# ---- [B] strategize.rs: recon_ready computation + branch + apply_contract_first_recon ----
say
say "== [B1] strategize.rs 100-240 (recon_ready + active_strategy branch) =="
sed -n '100,240p' crates/runtime_core/src/cortical_loop/strategize.rs | tee -a "$OUT"

say
say "== [B2] strategize.rs 340-440 (recovery check emission + proposal log) =="
sed -n '340,440p' crates/runtime_core/src/cortical_loop/strategize.rs | tee -a "$OUT"

say
say "== [B3] apply_contract_first_recon fn (find + dump) =="
awk '/fn apply_contract_first_recon/{p=1} p{print} p&&/^    }/{exit}' \
  crates/runtime_core/src/cortical_loop/strategize.rs | tee -a "$OUT"

# ---- [C] critic.rs: verdict function around the veto ----
say
say "== [C1] critic.rs 340-620 (verdict + veto + dialectic feedback) =="
sed -n '340,620p' crates/runtime_core/src/cortical_loop/critic.rs | tee -a "$OUT"

# ---- [D] critic_filter.rs: anti-loop recovery branch ----
say
say "== [D1] critic_filter.rs 60-230 (filter_anti_loop, FilterAndDecay, recovery) =="
sed -n '60,230p' crates/runtime_core/src/cortical/critic_filter.rs | tee -a "$OUT"

# ---- [E] recovery.rs: the write-injection functions ----
say
say "== [E1] recovery.rs 90-200 (after-READ_OK injection) =="
sed -n '90,200p' crates/runtime_core/src/cortical/recovery.rs | tee -a "$OUT"

say
say "== [E2] recovery.rs 470-560 (write_phase_ready branch) =="
sed -n '470,560p' crates/runtime_core/src/cortical/recovery.rs | tee -a "$OUT"

say
say "== [E3] recovery.rs 700-790 (write injection) =="
sed -n '700,790p' crates/runtime_core/src/cortical/recovery.rs | tee -a "$OUT"

say
say "== [E4] recovery.rs 900-940 (final write_phase_ready gate) =="
sed -n '900,940p' crates/runtime_core/src/cortical/recovery.rs | tee -a "$OUT"

# ---- [F] outcome_verifier.rs — the recon_ready comment context ----
say
say "== [F] outcome_verifier.rs 200-260 (recon_ready dependency) =="
sed -n '200,260p' crates/runtime_core/src/outcome_verifier.rs | tee -a "$OUT"

say
say "=============================================="
say "Probe2 log: $OUT  (paste this back)"
say "=============================================="
