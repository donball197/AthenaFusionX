# Verifier Audit Report

## Scope
Audited `crates/runtime_core/src/orchestrator_v2/verifier.rs` and `crates/runtime_core/src/completion_engine.rs`. MemoryPalace, CorticalLoop, and most other crates were not traced.

## verify_contract evidence marker parsing
`Verifier::verify_contract` (verifier.rs ~L100) delegates to `evaluate_contract_criteria` when criteria are non-empty; otherwise escalates (fail-closed). `read_evidence_matches_path` (verifier.rs ~L10) parses `[READ_OK]`, `[READ_CHUNK_OK] File:`, `[ACTION FAILED] path=`, and `[SEARCH_CODE_OK] Path:` markers, using `paths_equivalent`. Confirmed-absence reads are accepted only when `is_confirmed_absence` holds.

## Spoofing / partial / adversarial evidence
- `CMD_OK` criterion (verifier.rs ~L130) requires a `[CMD_OK]` line followed (within the same marker-bounded block) by `Status: exit status: 0`. A bare `[CMD_OK]` substring in read evidence does not satisfy it. CONFIRMED.
- `NODE_OBJECTIVE_EXECUTED_AND_VERIFIED` requires `[DAG NODE COMPLETE]`. CONFIRMED.
- `ANALYSIS_OK` requires `[HALT]`. CONFIRMED.
- TurnsExhausted evidence must not promote to Completed: no TurnsExhausted marker is treated as success anywhere in verify_contract; only explicit markers satisfy criteria. HYPOTHESIS/CONFIRMED that no promotion path exists in this file.

## command_ok_for semantics
`command_ok_for` (verifier.rs ~L78): normalizes command by stripping workspace-cd prefix and whitespace; takes first line of requested command; searches evidence for `[CMD_OK] Command:` whose normalized form contains the wanted normalized command, and within the following marker-bounded block a line `Status: exit status: 0`. Returns true if want is empty. CONFIRMED.

## extract_required_command semantics
`extract_required_command` (completion_engine.rs ~L230): handles backtick-delimited, quoted, colon-introduced, direct run/execute forms, known binaries, and raw shell leaders with quote-aware stopper truncation. CONFIRMED.

## infer_contract bypass by ambiguous directive
`infer_contract` (completion_engine.rs ~L470) first tries `declared_criteria_contract`; if criteria present, returns. Otherwise classifies by keyword intent. Ambiguous directives lacking explicit criteria fall through to intent heuristics; a directive containing semantic verbs may be classified as analysis and require `HaltDelivered`. Potential bypass: if an ambiguous directive contains no recognized intent words and no criteria, conditions may be empty leading to no contract (infer_contract always returns a contract; declared_criteria_contract returns None only when no conditions). HYPOTHESIS: ambiguous directive with neither criteria nor intent words yields a contract with possibly only CommandMarkerAcquired or empty conditions. RECOMMENDATION: require explicit criteria for completion.

## Not traced
MemoryPalace, CorticalLoop, and most crates were not traced.
