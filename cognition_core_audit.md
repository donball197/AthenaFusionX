# Cognition Core Audit

Scope: This audit is based only on the two files read in this node:
- `crates/nexus_substrate/src/memory.rs`
- `crates/runtime_core/src/cortical_loop.rs`

`bins/ide_backend`, the verifier, and most other crates were not traced. Findings are labeled `CONFIRMED`, `LIKELY`, `HYPOTHESIS`, or `RECOMMENDATION`. Line numbers below are 1-based positions in the files as read in this node.

## 1. MemoryPalace persistence and schema

### 1.1 Database initialization and migration behavior

- `MemoryPalace::new` (`memory.rs`, around the `pub async fn new` definition) opens SQLite with WAL journaling, `synchronous=Normal`, a 5-second busy timeout, cache size `-16000`, and a pool of up to 5 connections. It creates the DB file with `std::fs::File::create(db_path).ok()` when absent, then calls `init_full_schema` and `hydrate_vault`.
  - `CONFIRMED`: WAL mode and a busy timeout are present, which helps concurrent readers/writers.
  - `CONFIRMED`: `File::create(...).ok()` ignores filesystem creation errors. If creation fails, connection may still fail later, but the failure is not surfaced at the point of creation.
  - `RECOMMENDATION`: Surface or log DB-file creation failure instead of discarding it.

- `init_full_schema` (`memory.rs`) creates `neural_context`, `memory_search`, `system_insights`, `cognition_provenance`, and `plan_runs`, then performs migrations.
  - `CONFIRMED`: `ensure_column` is non-silent and propagates errors for targeted column additions.
  - `CONFIRMED`: Several legacy migrations use `.ok()` and therefore swallow errors:
    - `ALTER TABLE neural_context ADD COLUMN taint_label ...` (`memory.rs`, in `init_full_schema`)
    - `ALTER TABLE plan_runs ADD COLUMN root_directive ...`
    - `ALTER TABLE plan_runs ADD COLUMN graph_json ...`
  - `LIKELY`: These `.ok()` calls are intended to tolerate already-present columns, but they also hide unrelated SQLite errors such as locked DB, permission failure, or malformed schema.
  - `RECOMMENDATION`: Replace `.ok()` migrations with existence checks equivalent to `ensure_column`, or explicitly match duplicate-column errors.

- `ensure_column` (`memory.rs`) runs `PRAGMA table_info({table})` using string interpolation for the table name.
  - `CONFIRMED`: Table names are hardcoded call-site literals in this file, so there is no direct injection from external input in the observed code.
  - `HYPOTHESIS`: If `ensure_column` is reused elsewhere with untrusted table names, the interpolated SQL would be unsafe. This node did not trace other callers.

### 1.2 Memory write path

- `store_memory` (`memory.rs`) delegates to `store_memory_with_episode`.
- `store_memory_with_episode` (`memory.rs`) performs deduplication by `(origin, directive)`, then either updates the latest row or inserts a new row plus FTS row in a transaction.
  - `CONFIRMED`: Deduplication compares only `result`; if the result differs, a new row is inserted.
  - `CONFIRMED`: On duplicate result, taint labels are merged with `prev.join(taint_label)`, timestamp is refreshed, and embedding may be upgraded only if prior status is not `ready`.
  - `CONFIRMED`: The insert path uses a transaction for `neural_context` and `memory_search`, then commits before updating the in-memory `VectorVault`.
  - `CONFIRMED`: If `vault.commit_state` fails after DB commit, the DB row remains while the vector index may be stale. The error is logged but not returned.
  - `LIKELY`: This creates a recoverable inconsistency between SQLite and the vector vault after a vault commit failure.
  - `RECOMMENDATION`: Either make vault commit part of the transactional consistency strategy or add a repair/rehydration path for missing vector entries.

- Embedding validation:
  - `CONFIRMED`: `store_memory_with_episode` rejects embeddings whose length differs from `EMBEDDING_DIM` before insertion.
  - `CONFIRMED`: In the dedup upgrade path, `embedding.as_ref().filter(...)` silently skips invalid-length embeddings rather than returning an error.
  - `LIKELY`: This asymmetry means an invalid embedding can be accepted as "no upgrade" instead of failing the write.

- Concurrency:
  - `CONFIRMED`: SQLite pool max connections is 5 and busy timeout is 5 seconds.
  - `CONFIRMED`: `VectorVault` is protected by `parking_lot::Mutex`.
  - `CONFIRMED`: DB writes and vault updates are not atomic as a unit; DB commit occurs before vault commit.
  - `LIKELY`: Concurrent writers can interleave DB transactions safely at SQLite level, but vault state can lag or be updated out of order relative to DB rows.
  - `RECOMMENDATION`: Document or enforce a single ordering invariant between DB commit and vault commit, and add reconciliation on startup beyond `hydrate_vault`.

### 1.3 Recall and search

- `fetch_all`, `search_memories`, `semantic_search`, and random strand fetches read from SQLite and parse taint labels with `parse_taint`.
  - `CONFIRMED`: `parse_taint` fails closed to an untrusted/secret label for unparseable input.
  - `CONFIRMED`: Scoped recall filters by taint policy and episode scope.
  - `CONFIRMED`: `semantic_search` queries the vault first, then fetches each matched row individually from SQLite.
  - `LIKELY`: If a vault ID no longer exists in SQLite, it is silently skipped. This can mask vault/DB drift.
  - `RECOMMENDATION`: Count or log skipped vault IDs during semantic recall.

- FTS query construction in `search_memories` quotes tokens and joins with `OR`.
  - `CONFIRMED`: This avoids FTS5 control syntax injection from mission text.
  - `CONFIRMED`: Empty query returns no results.

## 2. Plan persistence, checkpointing, and recovery

### 2.1 Plan run schema and writes

- `upsert_plan_run` (`memory.rs`) inserts or updates a plan row, including `owner_pid` and `last_heartbeat`.
  - `CONFIRMED`: It always overwrites `owner_pid` and `last_heartbeat` with the current process ID and current timestamp on conflict.
  - `CONFIRMED`: It does not guard against overwriting a terminal row; an upsert can revive or alter a row by ID.
  - `LIKELY`: This is a significant concurrency/recovery hazard if two processes or stale callers use the same plan ID.
  - `RECOMMENDATION`: Restrict upsert conflict updates to non-terminal states, or require an expected status/version precondition.

- `update_plan_run_checkpoint` (`memory.rs`) updates `graph_json`, `status`, and `updated_at` only when:
  - row exists,
  - current status is `Active`,
  - new status is one of `Active`, `Completed`, or `Aborted`.
  - `CONFIRMED`: This prevents checkpoint writes to missing or non-Active plans.
  - `CONFIRMED`: It does not update `owner_pid` or `last_heartbeat`.
  - `CONFIRMED`: It returns `false` when no row is updated.
  - `LIKELY`: Callers must check the boolean; otherwise a failed checkpoint can be mistaken for success. This node did not trace callers.

- `heartbeat_plan_run` (`memory.rs`) touches `last_heartbeat` only for `Active` plans and does not modify graph/status.
  - `CONFIRMED`: This avoids clobbering terminal writes with a late heartbeat.
  - `CONFIRMED`: It ignores the number of affected rows and returns `Ok(())` even if the plan is missing or terminal.
  - `LIKELY`: A caller cannot distinguish "heartbeat recorded" from "plan no longer active."

- `abort_plan_if_active` (`memory.rs`) performs guarded `Active -> Aborted`.
  - `CONFIRMED`: It never touches terminal rows.
  - `CONFIRMED`: It returns whether a row was changed.

- `abort_plans_owned_by` (`memory.rs`) aborts all `Active` or `Suspended` plans for a given PID.
  - `CONFIRMED`: It is intended for SIGTERM/SIGINT cleanup.
  - `CONFIRMED`: It does not update `last_heartbeat`.
  - `LIKELY`: If invoked concurrently with recovery or another abort, SQLite transaction/locking governs correctness; no application-level lock is visible in this file.

### 2.2 Recovery semantics

- `recover_or_abort_orphaned_plans` (`memory.rs`) runs in a transaction and processes `Active`/`Suspended` rows ordered by `updated_at ASC`.
  - `CONFIRMED`: Owner liveness is determined by `/proc/{pid}` existence.
  - `CONFIRMED`: Process existence takes precedence over heartbeat age; a live owner with stale heartbeat is preserved and reported in `stale_heartbeat`.
  - `CONFIRMED`: A missing/invalid owner PID is treated as dead.
  - `CONFIRMED`: For dead owners, a non-empty parseable JSON graph is recorded as `recoverable`; the row is then marked `Aborted`.
  - `CONFIRMED`: There is no auto-resume path; recoverable plans are only reported.
  - `CONFIRMED`: Rows already terminal are excluded by the initial query.
  - `CONFIRMED`: The abort update is guarded by `status IN ('Active','Suspended')`, so a concurrent terminal transition prevents the abort.
  - `CONFIRMED`: `report.aborted` only includes rows where `rows_affected() == 1`.
  - `CONFIRMED`: `owner_pid` is read with `try_get(...).ok().flatten()`, so a missing column or null becomes `None`.
  - `LIKELY`: `/proc`-based liveness is Linux-specific and can be wrong in containers, PID namespaces, or if PIDs are reused.
  - `HYPOTHESIS`: A process that exists but is hung will be treated as live and its plan preserved indefinitely.
  - `RECOMMENDATION`: Add a maximum stale-heartbeat policy for live owners only if product requirements allow aborting hung owners; otherwise keep current conservative behavior and expose stale heartbeat to operators.

- `abort_orphaned_plans` (`memory.rs`) unconditionally aborts all `Active`/`Suspended` plans without ownership checks.
  - `CONFIRMED`: It is a broader, less selective recovery operation than `recover_or_abort_orphaned_plans`.
  - `LIKELY`: It should not be used when per-owner liveness matters.

- `recover_active_plans` (`memory.rs`) returns all `Active`/`Suspended` plans.
  - `CONFIRMED`: It does not filter by owner liveness or heartbeat.

### 2.3 SQL failure modes

- `CONFIRMED`: Most functions propagate `sqlx::Error`.
- `CONFIRMED`: `recover_or_abort_orphaned_plans` begins a transaction and commits at the end; if any query fails, the transaction is dropped and rolled back by sqlx semantics.
- `CONFIRMED`: `store_memory_with_episode` uses a transaction for the two inserts and commits before vault update.
- `CONFIRMED`: `upsert_plan_run` performs a single statement; partial failure is not applicable within one statement, but callers see the error.
- `CONFIRMED`: `rotate_provenance_ledger` uses a subquery to retain the newest `max_retain` rows; if `max_retain` is 0, the subquery behavior may delete all or none depending on SQLite semantics. This node did not test that edge case.
  - `HYPOTHESIS`: `max_retain = 0` may produce surprising retention because `LIMIT 0` yields no minimum timestamp.
  - `RECOMMENDATION`: Validate `max_retain > 0` or document the zero case.

## 3. CorticalLoop contract and completion semantics

### 3.1 Entry points

- `CorticalLoop::ignite` (`cortical_loop.rs`) delegates to `ignite_with_evidence(mission, None)`.
- `ignite_with_evidence` (`cortical_loop.rs`) mints an `EpistemicSession`, builds context, performs semantic recall, generates hypotheses, then runs a state machine.
  - `CONFIRMED`: It returns `CortexOutcome::CompletionProposed` or `CortexOutcome::TurnsExhausted`.
  - `CONFIRMED`: `CompletionProposed` carries `belief` and `evidence`.
  - `CONFIRMED`: `TurnsExhausted` carries `best_belief` and `evidence`.

### 3.2 Completion contract

- `TerminalGate::evaluate` is called with the current best hypothesis belief.
  - `CONFIRMED`: If `term_gate.is_autonomous_completion_satisfied()` is true, `ignite_with_evidence` immediately returns `CompletionProposed` with `belief = current_belief.max(0.95)` and evidence from `session.collect_evidence(best_node.id)`.
  - `CONFIRMED`: If the contract is satisfied but compile remediation is unresolved, it logs and continues rather than completing.
  - `CONFIRMED`: `evidence_satisfies_contract` evaluates the contract against `session.local_evidence` with belief probed at `1.0`.
  - `CONFIRMED`: `evidence_satisfies_contract` also requires `!requires_semantic_verification(&session.contract.objective)`.
  - `CONFIRMED`: In the no-hypothesis branch, if evidence satisfies the contract, it returns `CompletionProposed { belief: 0.95, evidence: session.local_evidence.clone() }`.
  - `CONFIRMED`: In the `CortexState::Fail` branch, if evidence satisfies the contract and no mutations were reverted, it returns `CompletionProposed { belief: belief.max(0.95), evidence: session.local_evidence.clone() }`.
  - `CONFIRMED`: If active mutations were reverted in the Fail branch, accumulated evidence is treated as stale and cannot satisfy the contract.
  - `LIKELY`: The contract can be satisfied by accumulated physical evidence even after hypothesis decay, which is an intentional recovery mechanism.
  - `RECOMMENDATION`: Ensure callers treat `CompletionProposed` as a proposal requiring downstream verification, not as an unconditional success, because this node did not trace the verifier.

### 3.3 Budget and failure behavior

- `session.budget.is_exhausted(turn)` controls turn termination.
  - `CONFIRMED`: High belief can extend the budget by 10 turns and 30 model calls when belief >= 0.80, fewer than 2 consecutive mutation vetoes, and turn < 80.
  - `CONFIRMED`: Otherwise budget exhaustion returns `TurnsExhausted`.
  - `CONFIRMED`: If compile remediation is active on budget exhaustion, `rollback_active_mutations` is called before returning.
  - `CONFIRMED`: `rollback_active_mutations` reverts all `session.active_mutation_paths`; if any revert fails, it keeps remediation active and records `[COMPILE_REVERT_FAILED]`.
  - `CONFIRMED`: On successful rollback, remediation state and active mutation paths are cleared.
  - `LIKELY`: A failed revert leaves source potentially broken and preserves remediation state, as intended.

### 3.4 Evidence inheritance

- `ignite_with_evidence` accepts `ancestor_evidence: Option<&str>`.
  - `CONFIRMED`: Non-empty ancestor evidence is added to `context_chunks`.
  - `CONFIRMED`: It is also hashed and appended to `session.local_evidence` as a DAG lineage envelope.
  - `CONFIRMED`: `inherit_physical_evidence` filters ancestor evidence to physical-evidence markers (`[READ_OK]`, `[READ_CHUNK_OK]`, `[CMD_OK]`, `[WRITE_OK]`, `[PATCH_OK]`, `[REPLACE_LINES_OK]`) and their following bodies, dropping other marker chatter.
  - `CONFIRMED`: The SHA-256 is computed over the filtered payload, not the raw ancestor evidence.
  - `CONFIRMED`: Tests in this file assert that unfiltered ancestor hashes do not validate and filtered hashes do.
  - `LIKELY`: This is a deliberate provenance mechanism to prevent stale or unfiltered evidence from satisfying downstream conditions.
  - `HYPOTHESIS`: The 120,000-character cap in `inherit_physical_evidence` could truncate large inherited evidence; this node did not trace whether truncation affects verifier expectations.

### 3.5 Concurrency and external mutation handling

- `ignite_with_evidence` subscribes to `self.dist.file_shift_tx`.
  - `CONFIRMED`: It drains pending file-shift events each turn.
  - `CONFIRMED`: Events are ignored if any executed action in `session.executed_action_log` touches the changed path, using `action_touches_path`.
  - `CONFIRMED`: Otherwise it records a `[CODE_SHIFT_ALERT]` in local evidence and observes a contradiction against the best hypothesis.
  - `LIKELY`: This handles concurrent external mutations by invalidating cached assumptions, but it does not automatically revert or reload files.
  - `RECOMMENDATION`: Trace `action_identity::action_touches_path` and the distributed scheduler to confirm event identity semantics; not done in this node.

### 3.6 Filesystem and command side effects

- `dump_debug` (`cortical_loop.rs`) creates `.nexus_debug` and writes turn payloads.
  - `CONFIRMED`: All filesystem errors are ignored with `let _ = ...`.
  - `CONFIRMED`: `resolve_workspace_paths` runs `cargo metadata` and ignores non-success or parse failures by returning diagnostic strings.
  - `LIKELY`: These are non-fatal diagnostics and do not affect completion correctness.

### 3.7 Memory recall integration

- `ignite_with_evidence` calls `memory.recall_semantic_context_scoped` with `RecallScope::CurrentMission` and `FlowPolicy::INTERNAL_SINK`.
  - `CONFIRMED`: Recall failure is non-fatal; execution continues without recalled experience.
  - `CONFIRMED`: Embedding failure is also non-fatal.
  - `CONFIRMED`: Recalled memories are advisory and cannot authorize completion.
  - `LIKELY`: This preserves availability when memory or embedding services fail.

## 4. Cross-cutting observations

- `CONFIRMED`: `MemoryPalace` combines SQLite persistence with an in-memory vector vault; consistency between them is best-effort, not transactional.
- `CONFIRMED`: Plan recovery is conservative for live owners and destructive only for dead owners.
- `CONFIRMED`: `CorticalLoop` can propose completion from physical evidence even when hypotheses decay, subject to compile-remediation and semantic-verification gates.
- `CONFIRMED`: This audit did not trace `bins/ide_backend`, the verifier, or most other crates, so downstream handling of `CompletionProposed`, recovery reports, and SQL errors is outside the scope of this report.

## 5. Recommended follow-ups

1. Make migration error handling explicit instead of `.ok()` where column existence is not first checked.
2. Add reconciliation or repair for vector vault drift after failed `commit_state`.
3. Make `upsert_plan_run` safe against terminal-row overwrite and concurrent same-ID writes.
4. Expose affected-row counts from `heartbeat_plan_run` if callers need liveness confirmation.
5. Trace callers of `update_plan_run_checkpoint`, `abort_plan_if_active`, and `recover_or_abort_orphaned_plans` to verify boolean results are handled.
6. Trace the verifier and `bins/ide_backend` to determine how `CompletionProposed` and `TurnsExhausted` are interpreted.
</content>