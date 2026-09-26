#!/usr/bin/env bash
set -u
cd ~/AthenaFusionX || exit 1
R=crates/runtime_core/src
D=crates/nexus_dag_orchestrator/src
B='=================================================================='
p(){ echo; echo "$B"; echo "[ $1 ]"; echo "$B"; }

p "math symbols excluding my own probes (exit 1 = truly absent)"
grep -rIn --exclude-dir=target --exclude-dir=.git --exclude='probe*.sh' \
  -e 'is_mathematical_derivation_directive' \
  -e 'create_mathematical_verification_graph' \
  -e 'mathematical_derivation_directive_is_detected' \
  -e 'deterministic_command_strategy' \
  -e 'exact_deterministic_command' \
  -e 'mathematical_verification' .
echo "grep exit: $?"

p "DAG planner 55-125 -- mentions_file_output through CRITICAL RULE 1"
sed -n '55,125p' "$D/planner.rs"

p "DAG executor/graph/lib -- ANY completion contract on a PlannedTask?"
rg -n 'criteria|complet|verif|WRITE_OK|artifact|satisf|done|finish|status' \
  "$D/executor.rs" "$D/graph.rs" "$D/lib.rs" "$D/main.rs"

p "strategy prefixes in cortical_loop -- exact-equality vs type-only"
rg -n 'strip_prefix|trim\(\) ==|matches!\(action' "$R/cortical_loop.rs"

p "extract_required_command call site -- what text does it receive?"
sed -n '325,348p' "$R/completion_engine.rs"

p "every write-signal predicate in the tree"
rg -n 'mentions_file_output|write_words|is_write_tgt|has_write_signal|has_uptime' --glob '*.rs' .
