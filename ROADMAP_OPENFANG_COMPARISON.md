# OpenFang vs AthenaFusionX Architectural Comparison

Tags used throughout:
- **[V]** verified from source output captured in the audit session (file:line given)
- **[D]** documented claim (README/docs) not confirmed in code
- **[A]** analysis or recommendation (not a fact about OpenFang)
- **[N]** not inspected

OpenFang commit: cloned into `vendor/openfang`; workspace version `0.6.9` [V] (`Cargo.toml`, `[workspace.package]`). Re-cloned checkout is commit `acf2587e46be174c10200489c9a2d23a39a98aeb`. Line numbers for items read before the re-clone (sandbox.rs, substrate.rs, migration.rs, audit.rs, etc.) came from an earlier clone that may have been a different commit; re-check them against this hash.
AthenaFusionX internals (VmoHandler, MemoryPalace, VectorVault, etc.) were **not** re-read in this audit; statements about them come from the mission brief and are tagged [N].

## Executive Summary

- OpenFang's workspace has 14 members: 13 crates plus `xtask` [V]. The "14 crates" figure is slightly off; `xtask` is a build-automation stub that prints "no tasks defined yet" [V].
- WASM sandbox: wasmtime with fuel metering and epoch interruption both enabled, a default fuel budget of 1,000,000, and a 30 s wall-clock watchdog [V].
- Memory: SQLite in WAL mode with `busy_timeout=5000` [V]; a `memories` table with an `embedding BLOB` column and cosine-similarity recall, falling back to `LIKE` matching [V per module docs; recall body not read].
- Hands: 9 bundled hands [V]; hand manifest hashes are reported into the audit hash chain [V, doc comment].
- "16 security layers": documented in the README as a 16-row table [D]. Layers 1-6, 13 and 16 are confirmed in code (8 of 16), 7 partially, 9/12/14 at file level; 8, 10, 11, 15 are unverified (Section 6).
- Caution: `openfang security status` prints hard-coded strings [V], so it is not evidence of enforcement.

## 1. Architecture Mapping

| OpenFang | AthenaFusionX [N] | Purpose | Key difference | Lesson [A] |
|---|---|---|---|---|
| `openfang-kernel` + `openfang-runtime` (+ `openfang-types`) | `nexus_runtime_core` / `nexus_substrate` | Agent lifecycle, execution, tool running | OpenFang splits kernel (registry, wizard) from runtime (sandbox, tool_runner, audit) [V]; crate-level dependency edges not read | Compare crate boundaries once `Cargo.toml` per crate is read |
| `openfang-runtime/src/sandbox.rs` (wasmtime) | `VmoHandler` / ARM64 execution boundary | Contain untrusted tool code | OpenFang isolates guest code in WASM; Nexus boundary mechanism differs and was not compared line-by-line | Per-execution budget plus wall-clock kill is separable from WASM |
| `openfang-memory` (SQLite WAL, `memories`, cosine recall) | `MemoryPalace` / `VectorVault` / `SqliteCheckpointer` | Persistent memory and retrieval | OpenFang: BLOB embeddings + cosine; no FTS5 seen in captured output | Pragma set is directly comparable |
| `openfang-hands` (`HandRegistry`, bundled hands) + kernel | Executive Cortex → DAG → Subagent → MotorCommand → RoboticExecutor | Packaged autonomous capabilities | Hands are declarative packages; Nexus builds a DAG per goal | Audit-hash of loaded manifests |

## 2. OpenFang Workspace and Kernel

Members [V, `Cargo.toml`]: `openfang-types`, `openfang-memory`, `openfang-runtime`, `openfang-wire`, `openfang-api`, `openfang-kernel`, `openfang-cli`, `openfang-channels`, `openfang-migrate`, `openfang-skills`, `openfang-desktop`, `openfang-hands`, `openfang-extensions`, `xtask`.

Observed separation [V]:
- **Types**: `openfang-types` holds shared types (agent manifest, config, memory trait, taint, commands).
- **Kernel**: `openfang-kernel` has `AgentRegistry` (DashMap indexes by id, name, tag) and `SetupWizard`; `OpenFangKernel::boot_with_config` is used by the API integration tests.
- **API**: the API tests boot a real kernel behind an axum server (`api_integration_test.rs`).
- **Memory**: `openfang-memory` owns SQLite.
- **Runtime**: `openfang-runtime` owns the WASM sandbox, `tool_runner`, `audit`, `web_fetch`, and `provider_health`.
- **Wire**: `openfang-wire` implements the peer node (TCP listener, handshake, dispatch loop).

[N] Per-crate dependency edges. [A] No claim is made about which layer depends on which beyond the imports above.

## 3. WASM Sandboxing and Execution Security

All [V] from `crates/openfang-runtime/src/sandbox.rs`:

- **Engine config** (112-113): `consume_fuel(true)` and `epoch_interruption(true)`. One `Engine` per kernel, reused.
- **Fuel** (37, 53, 178-181): `fuel_limit: u64`, default `1_000_000`; `set_fuel` is applied only when `fuel_limit > 0`.
- **Epoch** (184-191): `set_epoch_deadline(1)`; a spawned `std::thread` sleeps `timeout_secs` (default 30) and then calls `engine.increment_epoch()`.
- **On limit exceeded** (242-252): `Trap::OutOfFuel` maps to `SandboxError::FuelExhausted`. `Trap::Interrupt` maps to `SandboxError::Execution("timed out after Ns (epoch interrupt)")`.
- **Guest/host boundary**:
  - Instantiated through a `Linker` with host functions registered; the code comment says "no WASI".
  - The guest must export `memory`, `alloc(i32)->i32`, and `execute(i32,i32)->i64`.
  - Input is bounds-checked before copy into guest memory.
  - Output is a packed i64 (ptr high 32, len low 32).
  - Execution is offloaded to a blocking thread (doc comment).
- **Reported fuel use** (274): `fuel_consumed = fuel_limit - fuel_remaining`.

Observations [A] from the code shown:
- The watchdog thread is not joined or cancelled, so each execution leaves a thread sleeping until its timeout.
- No memory-growth limit (e.g. `StoreLimits`) appeared in the captured excerpts. [N] not exhaustively searched.

[N] `register_host_functions` body, so the per-call capability check claimed in the doc comment is not verified.

Comparison with Nexus [N/A]: OpenFang's boundary is a WASM VM with instruction-level metering; a native ARM64 executor boundary is a different mechanism, and no equivalence is claimed.

## 4. Memory, SQLite WAL, and Vector Retrieval

[V] unless noted:
- **WAL**: `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;` (`openfang-memory/src/substrate.rs:52`).
- **Tables** (`migration.rs`): `agents`, `sessions`, `events`, `kv_store`, `task_queue`, `memories`, `entities`, `relations`, `migrations`, `usage_events`, `canonical_sessions`, `paired_devices`, `audit_entries`. `audit_entries` is also created in `openfang-runtime/src/audit.rs:379`, which may be test setup [N].
- **`memories` columns**: `id, agent_id, content, source, scope (default 'episodic'), confidence, metadata, created_at, accessed_at, access_count, deleted`, plus `embedding BLOB` added in migration v3 (`migration.rs:215-224`).
- **Retrieval** (`semantic.rs` module docs): with a query embedding, recall ranks by cosine similarity over stored BLOB embeddings; without, it falls back to SQLite `LIKE`. `remember_with_embedding(..., Option<&[f32]>)` stores vectors. An alternative remote backend (PostgreSQL + pgvector + Jina) is mentioned in a doc comment (`semantic.rs:27`); `lib.rs:5` mentions Qdrant as "Phase 2".
- **Checkpointing**: a search for `checkpoint` in `openfang-memory/src` returned no matches. No explicit `wal_checkpoint` was found.
- **Migrations**: versioned `migrations` table with `column_exists` guards.
- **Isolation/taint around memory**: `memories.agent_id` scoping exists in schema; enforcement code [N]. Taint types live in `openfang-types/src/taint.rs` and are used in `openfang-runtime/src/tool_runner.rs` (shell_exec and net_fetch sinks, Section 6).

[N] Recall implementation (brute-force scan vs index), embedding dimensionality, FTS5 use.

Nexus side [N]: `MemoryPalace`/FTS5, `VectorVault`/`TurboQuantIndex`, `granite-311m-edge` 384-d embeddings, and `SqliteCheckpointer` were not inspected here.

## 5. Hands vs Executive Cortex / DAG

OpenFang [V unless noted]:
- `HandRegistry` holds `instances: DashMap<Uuid, HandInstance>` (`openfang-hands/src/registry.rs:59-60`).
- Each successful HAND.toml load or reload computes SHA-256 of the raw file and calls a callback that the kernel wires into the audit hash chain (`registry.rs:17-28, 61-63`, issue #1172; the doc comment calls it a Merkle chain).
- `HandCategory` includes Content, Security, Productivity, Development, Data (`lib.rs:43-61, 484-488`).
- Bundled hands: `browser, clip, collector, infisical-sync, lead, predictor, researcher, trader, twitter` (`openfang-hands/bundled/`).
- `SetupWizard` turns an `AgentIntent` (name, task, skills, model_tier, scheduled, schedule cron/interval, capabilities) into an `AgentManifest` with `ManifestCapabilities` (network, tools), `Priority`, `ResourceQuota`, `ScheduleMode` (`wizard.rs`).
- HAND.toml is not at the repo root; per-hand files inside `bundled/*` were not opened [N].
- [N] Kernel scheduler implementation.

Similarities [A]: both turn a natural-language or declared intent into a structured plan with capability limits. Differences [A]: Hands are static, declarative, and long-lived; the Nexus Cortex builds a per-goal DAG and executes commands through a verification loop. No ranking is implied.

## 6. Security-Layer Verification

The README table (`README.md:206-227`) lists 16 systems [D]. Code corroboration from the audit:

| # | Layer (README) | Code evidence found |
|---|---|---|
| 1 | WASM dual-metered sandbox | Yes: `sandbox.rs:112-113,178-191,242-252` [V] |
| 2 | Merkle hash-chain audit | **Yes** [V]: `openfang-runtime/src/audit.rs` defines `AuditEntry` (seq, timestamp, agent_id, action, detail, outcome, prev_hash, hash), `compute_entry_hash` (SHA-256 over all fields), `with_db` (:109), `record` (:180), `verify_integrity` (:241); bodies read (see 7.2). Linear hash chain, not a Merkle tree. |
| 3 | Taint tracking | **Yes (types)** [V, `taint.rs`]: 5 `TaintLabel`s; `TaintedValue` with `merge_taint`, `check_sink`, `declassify`. Each `TaintSink` is a blocklist of labels: `shell_exec` blocks ExternalNetwork, UntrustedAgent, UserInput; `net_fetch` blocks Secret, Pii; `agent_message` blocks Secret. `check_sink` returns the first blocked label found (HashSet order). `declassify` just removes a label. Call sites [V, `tool_runner.rs`]: `check_sink(&TaintSink::shell_exec())` (:51) and `check_sink(&TaintSink::net_fetch())` (:78); no `agent_message` sink reference in that file. `merge_taint` body [N]. |
| 4 | Ed25519 signed manifests | **Yes** [V]: `openfang-types/src/manifest_signing.rs` (`SignedManifest`: manifest, SHA-256 content_hash, Ed25519 signature, signer public key, signer_id; `ed25519-dalek`). Verify function body and whether the kernel pins trusted keys [N]. |
| 5 | SSRF protection | **Yes** [V, `openfang-runtime/src/web_fetch.rs`]: `check_ssrf(url, allowed_hosts)` (:195) and `is_private_ip` (:352), with tests for localhost, private IPs, cloud metadata, non-http schemes, 0.0.0.0 and IPv6 loopback (:427-478). DNS-rebinding defence [N/D]. |
| 6 | Secret zeroization | **Yes (vault)** [V]: `openfang-extensions/src/vault.rs` stores entries as `Zeroizing<String>` and the master key as `Zeroizing<[u8;32]>`. Coverage of provider API keys elsewhere [N]. |
| 7 | OFP mutual auth | **Partial** [V]: `openfang-wire/src/peer.rs` uses HMAC-SHA256 (:16,26,79) and constant-time compare via `subtle::ConstantTimeEq::ct_eq` (:87). Nonce/handshake flow [N]. |
| 8 | Capability gates | [D] only (`openfang-runtime/src/host_functions.rs` exists) |
| 9 | Security headers | **File level** [V]: CSP/HSTS/X-Frame pattern matched `openfang-api/src/middleware.rs` and `webchat.rs`. Which headers are set [N]. |
| 10 | Health endpoint redaction | [D] only |
| 11 | Subprocess sandbox | [D] only (search output was truncated before runtime files) |
| 12 | Prompt injection scanner | **File level** [V]: pattern `injection` (with `env_clear|canonicalize`) matched `openfang-skills/src/{verify,registry,loader,installer,lib,openclaw_compat}.rs` and `openfang-types/src/{taint,webhook,config}.rs`, `openfang-memory/src/session.rs`. Which pattern hit where [N]. |
| 13 | Loop guard | **Yes** [V, `openfang-runtime/src/loop_guard.rs`]: `LoopGuard::check(tool_name, params) -> LoopGuardVerdict` (:146), `record_outcome` (:251), `get_poll_backoff` (:287), `compute_hash(tool_name, params) -> String` (:501), `LoopGuardConfig`/`LoopGuardStats`. Hash algorithm, ping-pong detection, circuit-breaker behaviour [N]. |
| 14 | Session repair | **File exists** [V]: `openfang-runtime/src/session_repair.rs`. Logic [N]. |
| 15 | Path traversal prevention | [D] only |
| 16 | GCRA rate limiter | **Yes (cost-aware limiter)** [V, `openfang-api/src/rate_limiter.rs`]: `operation_cost(method, path) -> NonZeroU32` (:14), `create_rate_limiter() -> Arc<KeyedRateLimiter>` (:51). Whether the algorithm is GCRA and per-IP keying [N]. |

Result: the "16 distinct layers" claim **is documented**. Confirmed as real code: 1, 2, 3, 4, 5, 6, 13, 16 (8 layers). Partial: 7. File level only: 9, 12, 14. README only: 8, 10, 11, 15. Confirmed means the mechanism exists, not that every call path enforces it. `cmd_security_status` prints hard-coded constants (`openfang-cli/src/main.rs:6316-6345`), and `routes.rs:6171` reports `"epoch_interruption": true` as a value; neither proves enforcement.

## 7. Three Strategic Assimilation Targets

### 7.1 Dual metering (fuel + wall-clock epoch) per execution
- **Source**: `openfang-runtime/src/sandbox.rs:112-113, 178-191, 242-252`.
- **Does**: caps deterministic work (fuel) and wall-clock time (epoch watchdog) per call, and returns distinct errors for each.
- **Purpose**: reliability and DoS resistance against runaway execution.
- **Nexus mapping [A]**: a per-MotorCommand budget (work or step cap plus wall-clock cap) at the RoboticExecutor boundary, with separate failure classes for "budget" and "timeout".
- **Disposition**: **adapt**. The WASM-specific parts do not transfer; the two-limit pattern does. Fix the un-joined watchdog thread if copying.

### 7.2 Hash-chained audit with manifest-content hashing
- **Source**: `openfang-hands/src/registry.rs:17-28`; `migration.rs` (`audit_entries`); `openfang-runtime/src/audit.rs`.
- **Does**: hashes each loaded HAND.toml (SHA-256) and feeds the digest into a tamper-evident chain.
- **Purpose**: integrity and traceability of what the system loaded and did.
- **Nexus mapping [A]**: chain the evidence records behind each DAG node (command, output hash, file hash) so a node cannot be marked verified without matching evidence. This addresses the failure seen in the last run, where a corrupt roadmap file passed verification.
- **Code facts** [V, `audit.rs`]:
  - The structure is a linear hash chain (each entry hashes the previous), not a Merkle tree, despite the name. Entry hash = SHA-256 over seq, timestamp, agent_id, action, detail, outcome, prev_hash, fed to the hasher back to back with no delimiters or length prefixes.
  - `record` locks entries and tip, appends in memory, and persists with `let _ = conn.execute(INSERT ...)`, so DB write errors are silently ignored.
  - `verify_integrity` walks from the all-zero genesis, checks `prev_hash` linkage, recomputes every hash, and returns the first inconsistency.
  - `with_db` reloads all rows ordered by seq and runs `verify_integrity` at boot; on failure it only logs `tracing::error!` and continues.
  - Unknown action strings on reload fall back to `ToolInvoke`. `tip_hash()` exposes the tip.
- **Note [A]**: without delimiters, field boundaries are ambiguous (bytes moved between `detail` and `outcome` hash identically). Length-prefix or delimit fields in any Nexus copy.
- **Limits [A]**: dropping the newest entries leaves a valid chain unless the tip hash is anchored externally; boot verification does not halt; persistence errors are invisible. A Nexus version should anchor the tip (e.g. in the DAG plan record), surface write failures, and fail closed.
- **Disposition**: **adapt**.

### 7.3 SQLite WAL with busy timeout
- **Source**: `openfang-memory/src/substrate.rs:52`.
- **Does**: `journal_mode=WAL` and a 5 s `busy_timeout` at connection open.
- **Purpose**: concurrent reads during writes and fewer "database is locked" errors.
- **Nexus mapping [A]**: apply the same pragmas wherever `SqliteCheckpointer`/`MemoryPalace` open connections, if not already set.
- **Disposition**: **adopt** (after confirming Nexus does not already set them).

## 8. Zero Fragmentation Implications

[A] The term is Nexus's own; its formal definition was not in the audit evidence. Working interpretation: no duplicate subsystems.
- Add budgets to the existing executor, not a second runtime.
- Extend the existing checkpointer and memory stores instead of adding a parallel store.
- Any audit chain should attach to the existing evidence-marker path.
- Do not vendor OpenFang crates into the build; `vendor/openfang` is reference only.

## 9. Evidence and Source Paths

All under `vendor/openfang/`:
- `Cargo.toml` (workspace members, version)
- `crates/openfang-runtime/src/sandbox.rs`
- `crates/openfang-memory/src/substrate.rs`, `migration.rs`, `semantic.rs`, `lib.rs`
- `crates/openfang-runtime/src/audit.rs`, `tool_runner.rs`, `web_fetch.rs`, `host_functions.rs`, `loop_guard.rs`, `session_repair.rs`, `agent_loop.rs`; `crates/openfang-api/src/rate_limiter.rs`, `middleware.rs`; `crates/openfang-skills/src/verify.rs`
- `crates/openfang-types/src/taint.rs`, `manifest_signing.rs`, `serde_compat.rs`, `memory.rs`, `config.rs`
- `crates/openfang-extensions/src/vault.rs`, `crates/openfang-wire/src/peer.rs`
- `crates/openfang-hands/src/registry.rs`, `lib.rs`, `bundled/`
- `crates/openfang-kernel/src/registry.rs`, `wizard.rs`
- `crates/openfang-cli/src/main.rs` (lines ~6316-6345)
- `crates/openfang-api/src/routes.rs` (line 6171), `tests/api_integration_test.rs`
- `README.md` (lines 193, 206-227)

## 10. Recommended Next Engineering Steps

1. Read the remaining unread pieces: `merge_taint` (`taint.rs` 77-85), whether `TaintSink::agent_message` is used anywhere (not in `tool_runner.rs`), the `verify` function in `manifest_signing.rs` (and where the kernel decides which signer keys to trust), `register_host_functions` in `sandbox.rs`, the OFP nonce handshake in `peer.rs`, and the `recall` body in `semantic.rs`.
2. Close the open layers with one targeted `rg` per layer: 8 (capability gates), 10 (health redaction), 11 (`env_clear`), 15 (path traversal), plus the internals of 7, 9, 12, 14, 16 (GCRA/keying). Update Section 6 after each.
3. Check Nexus connection setup for `journal_mode=WAL` and `busy_timeout`; apply target 7.3 if missing.
4. Prototype target 7.1 at the executor boundary behind a config flag.
5. Prototype target 7.2 on evidence markers, after step 1.
6. Harden the Nexus verifier before the next DAG audit: reject `COMPLETED_UNVERIFIED`, and check generated files structurally (header present, at least 10 `## ` sections, no embedded tool-call JSON).
