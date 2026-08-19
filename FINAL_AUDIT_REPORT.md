# OpenFang Architecture Report

## System Overview

OpenFang is a sovereign agent runtime built on a **capability-secure WASM sandbox** with a **cognitive loop** for autonomous reasoning. The architecture separates concerns into distinct crates:

| Crate | Responsibility |
|-------|----------------|
| `openfang-runtime` | WASM sandbox execution, host ABI, capability enforcement |
| `openfang-memory` | SQLite-backed structured storage (KV + agent persistence) |
| `openfang-types` | Core types: capabilities, taint tracking, agent manifests |
| `runtime_core` | Cognitive loop, routing, completion verification, execution |

The system follows a **deny-by-default** security model: WASM guests have zero ambient authority. Every host call is capability-checked. Data flowing through the system carries **taint labels** that prevent exfiltration and injection.

---

## Core Components

### 1. WASM Sandbox (`openfang-runtime/src/sandbox.rs`)

**Purpose**: Execute untrusted WASM modules (skills/plugins) with deterministic resource limits and capability-based access control.

#### Guest ABI (Module Exports)
| Export | Signature | Purpose |
|--------|-----------|---------|
| `memory` | `Memory` | Linear memory shared with host |
| `alloc` | `(i32) -> i32` | Allocate bytes in guest memory, return pointer |
| `execute` | `(i32, i32) -> i64` | Entry point: receives input ptr/len, returns packed result ptr/len |

#### Host ABI (Imports under `"openfang"` module)
| Import | Signature | Purpose |
|--------|-----------|---------|
| `host_call` | `(i32, i32) -> i64` | RPC dispatch: request JSON → response JSON (packed) |
| `host_log` | `(i32, i32, i32) -> ()` | Structured logging (level, msg_ptr, msg_len) |

#### Execution Flow

execute(wasm_bytes, input_json, config, kernel, agent_id)
    │
    ├─► spawn_blocking (offloads CPU-bound WASM from Tokio)
    │
    ├─► Module::new(engine, wasm_bytes)  // Compile (accepts .wasm + .wat)
    │
    ├─► Store::new(engine, GuestState)   // GuestState carries capabilities, kernel, agent_id
    │
    ├─► set_fuel(config.fuel_limit)      // Deterministic CPU metering
    │
    ├─► set_epoch_deadline(1) + watchdog thread  // Wall-clock timeout via epoch interruption
    │
    ├─► Linker::new + register_host_functions()
    │       ├─► host_call → host_functions::dispatch(capability-checked)
    │       └─► host_log  → tracing (no capability check)
    │
    ├─► linker.instantiate(&mut store, &module)
    │
    ├─► Get exports: memory, alloc, execute
    │
    ├─► Serialize input → alloc → write to guest memory
    │
    ├─► execute_fn.call(input_ptr, input_len) → packed i64
    │
    ├─► Unpack result_ptr, result_len → read from guest memory
    │
    ├─► Parse output JSON
    │
    └─► Calculate fuel_consumed = fuel_limit - fuel_remaining


#### Resource Enforcement
| Mechanism | Configuration | Trap on Exhaustion |
|-----------|---------------|-------------------|
| **Fuel** | `SandboxConfig.fuel_limit` (default 1M) | `Trap::OutOfFuel` → `SandboxError::FuelExhausted` |
| **Epoch/Wall-clock** | `SandboxConfig.timeout_secs` (default 30s) | `Trap::Interrupt` → timeout error |
| **Memory** | `max_memory_bytes` (reserved, not yet enforced) | — |

#### Capability Model
- `GuestState.capabilities: Vec<Capability>` checked in `host_functions::dispatch` before every host call
- Capabilities granted via `SandboxConfig.capabilities` at instantiation time
- No ambient authority: empty capabilities = no host calls permitted

#### SSRF Protection
- `ssrf_allowed_hosts` from config forwarded to `GuestState`
- Host fetch operations check against this allowlist before connecting to private IPs

---

### 2. Structured Store (`openfang-memory/src/structured.rs`)

**Purpose**: SQLite-backed persistence for agent state and per-agent key-value storage.

#### Schema (implied by queries)
sql
-- Key-value store per agent
CREATE TABLE kv_store (
    agent_id TEXT,
    key TEXT,
    value BLOB,           -- JSON serialized as bytes
    version INTEGER,
    updated_at TEXT,
    PRIMARY KEY (agent_id, key)
);

-- Agent entries
CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT,
    manifest BLOB,        -- MessagePack (rmp_serde) with named fields
    state TEXT,           -- JSON string
    created_at TEXT,
    updated_at TEXT,
    session_id TEXT DEFAULT '',
    identity TEXT DEFAULT '{}'
);


#### Key Operations
| Method | SQL Pattern | Notes |
|--------|-------------|-------|
| `get` | `SELECT value FROM kv_store WHERE agent_id=? AND key=?` | Returns `Option<serde_json::Value>` |
| `set` | `INSERT ... ON CONFLICT DO UPDATE` | Auto-increments version, updates timestamp |
| `delete` | `DELETE FROM kv_store WHERE agent_id=? AND key=?` | |
| `list_kv` | `SELECT key, value FROM kv_store WHERE agent_id=? ORDER BY key` | |
| `save_agent` | `INSERT ... ON CONFLICT(id) DO UPDATE` | Handles schema migrations via `ALTER TABLE` |
| `load_agent` | `SELECT ... FROM agents WHERE id=?` | Graceful fallback for missing columns |
| `load_all_agents` | `SELECT ... FROM agents` | **Auto-repair**: re-serializes manifests with current schema, deduplicates by name |

#### Migration Strategy
- `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` executed on every `save_agent`
- `load_agent` / `load_all_agents` try progressively simpler queries (with/without `session_id`, `identity`)
- `load_all_agents` detects schema drift: if re-serialized manifest differs from stored blob, queues `UPDATE` to upgrade

#### Concurrency
- `Arc<Mutex<Connection>>` — coarse-grained locking, suitable for low-contention agent workloads

---

### 3. Taint Tracking (`openfang-types/src/taint.rs`)

**Purpose**: Lattice-based information flow control to prevent confused-deputy attacks, prompt injection, and data exfiltration.

#### Taint Labels (Security Lattice)

ExternalNetwork  ──┐
UserInput          ├──► Blocked by shell_exec
UntrustedAgent     ┘
Secret             ──┐
Pii                └──► Blocked by net_fetch, agent_message


#### Core Types
rust
struct TaintedValue {
    value: String,
    labels: HashSet<TaintLabel>,
    source: String,  // Human-readable provenance
}

struct TaintSink {
    name: String,
    blocked_labels: HashSet<TaintLabel>,
}


#### Predefined Sinks
| Sink | Blocked Labels | Threat Mitigated |
|------|----------------|------------------|
| `shell_exec()` | ExternalNetwork, UntrustedAgent, UserInput | Command injection |
| `net_fetch()` | Secret, Pii | Data exfiltration |
| `agent_message()` | Secret | Lateral secret leakage |

#### Operations
- `merge_taint(other)` — Union of labels (for concatenation/composition)
- `check_sink(sink)` — Returns `Err(TaintViolation)` if any label blocked
- `declassify(label)` — Explicit removal (audited security decision)
- `is_tainted()` — Any label present

#### Usage Pattern
rust
// Before sensitive operation:
tainted_value.check_sink(&TaintSink::shell_exec())?;
// Proceed only if Ok(())


---

### 4. Intelligent Router (`runtime_core/src/router.rs`)

**Purpose**: Classify incoming directives into `DirectiveClass` and produce an `AgentProfile` with model selection, budget, and persona.

#### Directive Classification
rust
fn classify(d: &str) -> DirectiveClass {
    let d = d.to_lowercase();
    if ["diagnose", "doctor", "fix repo", "suture"].iter().any(|k| d.contains(k)) {
        DirectiveClass::RepoDoctor
    } else if ["cargo check", "ls ", "find "].iter().any(|k| d.contains(k)) {
        DirectiveClass::ShellOp
    } else if ["fix", "patch", "edit"].iter().any(|k| d.contains(k)) {
        DirectiveClass::TargetedEdit { file_hint: "target".into() }
    } else {
        DirectiveClass::General
    }
}


#### AgentProfile Configuration
| Class | Model (env fallback) | Budget | Max Turns |
|-------|---------------------|--------|-----------|
| `RepoDoctor` / `SecurityAudit` | `NEXUS_HEAVY_MODEL` → `nvidia/nemotron-3-ultra` | `doctor()` (15K chars, 8K tokens) | 8 |
| Others | `NEXUS_LIGHT_MODEL` → `gemini-3.5-flash-lite` | `standard()` (8K chars, 4K tokens) | 8 |

#### Persona
Fixed system prompt enforcing:
- **Single JSON array output** (no markdown)
- **Strict action schema** (8 action types)
- **Patch discipline**: `patch_function`/`diff_patch` over `write_file` for modifications
- **Halt override**: Must verify physical evidence (`[PATCH_FUNCTION_OK]`, etc.) before halting

---

### 5. Cortical Loop (`runtime_core/src/cortical_loop.rs`)

**Purpose**: Main cognitive loop — hypothesis generation, criticism, action translation, execution, evaluation, and completion verification.

#### Architecture

┌─────────────────────────────────────────────────────────────┐
│                      CorticalLoop                           │
├─────────────────────────────────────────────────────────────┤
│  WorldModel  │  MemoryCortex  │  VmoState  │  DistributedScheduler  │
└──────────────┴────────────────┴────────────┴──────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │ EpistemicSession│
                    │  (per-mission)  │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
       ┌────────────┐ ┌────────────┐ ┌────────────┐
       │ Hypothesis │ │  Critic    │ │ Evaluator  │
       │  Generator │ │ (Red Team) │ │ (Evidence) │
       └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
             │              │              │
             └──────────────┼──────────────┘
                            ▼
                   ┌─────────────────┐
                   │ MotorCortex →   │
                   │ RoboticExecutor │
                   └────────┬────────┘
                            ▼
                   ┌─────────────────┐
                   │  Evidence Log   │
                   │ (local_evidence)│
                   └────────┬────────┘
                            │
                            ▼
                   ┌─────────────────┐
                   │ Completion Guard│
                   │ (evaluate_      │
                   │  _completion)   │
                   └─────────────────┘


#### Turn Loop
1. **Select best hypothesis** (highest belief score)
2. **Budget check** — `CognitiveBudget.is_exhausted(turn)` with **momentum guard**: extends budget if belief ≥ 0.55
3. **Completion evaluation** — `evaluate_completion()` derives `CompletionState` from `session.contract`
4. **Strategist proposes action** → `EpistemicEngine::propose_action`
5. **Anti-loop guard** — Rejects repeated actions unless contract satisfied
6. **Critic reviews** → `EpistemicEngine::criticize_proposal`
   - **Circuit breaker**: Overrides critic after 3 consecutive vetoes of mutation proposals while contract unmet
7. **Translator** → JSON actions (direct intercept or LLM translation)
8. **ActionGate approval** — Capability check
9. **MotorCortex → RoboticExecutor** — Physical execution
10. **Evidence recorded** → `session.local_evidence`
11. **Evaluator scores evidence** → `EpistemicEngine::evaluate_evidence`
    - **Auto-revert**: Negative score on mutation → `git checkout -- <path>` + strip evidence
12. **Belief propagation** in cognitive graph
13. **Halt intercept** — If `Halt` action: **Semantic Verifier** (`EpistemicEngine::verify_outcome`) must approve

#### Key Guards (Permanent Architectural Fixes)
| Guard | Trigger | Behavior |
|-------|---------|----------|
| **Cognitive Momentum** | Budget exhausted, belief ≥ 0.55 | +20 turns, +80 model calls |
| **Anti-Loop** | Repeated action key | Veto unless contract satisfied |
| **Circuit Breaker** | 3× critic veto on mutation proposal | Force execution |
| **Auto-Revert** | Evaluator confidence < 0 on mutation | `git checkout`, strip evidence |
| **Semantic Verifier** | `Halt` action | LLM verifies goal satisfaction; rejects if incomplete |
| **Synthesis Lock** | ~~Mission contains "SYNTHESIS"/"DOCUMENT"~~ | **REMOVED** — caused false positives |

#### Completion State (from `evaluate_completion`)
rust
struct CompletionState {
    is_write_task: bool,           // Contract has WriteMarkerAcquired
    is_mutation_task: bool,        // Contract.is_mutation_task
    has_prior_evidence: bool,      // Evidence markers present
    compound_satisfied: bool,      // All contract conditions met
}

- Derived from `session.contract` (single source of truth), not raw mission text
- Mutation tasks require `[WRITE_OK]`/`[PATCH_FUNCTION_OK]`/`[DIFF_PATCH_OK]`
- Read tasks require `[READ_OK]`/`[SEARCH_CODE_OK]`
- Compound objectives (AND/OR) evaluated against evidence keywords

---

### 6. Completion Engine (`runtime_core/src/completion_engine.rs`)

**Purpose**: Infer and enforce **subgoal contracts** — explicit completion conditions per cognitive node.

#### Contract Inference (`infer_contract`)
rust
fn infer_contract(node_id, objective) -> SubgoalContract {
    // Negation-aware classification (critical fix)
    if any_word_unnegated(text, WRITE_WORDS) {
        conditions.push(WriteMarkerAcquired);
        is_mutation_task = true;
    } else if any_word_unnegated(text, PATCH_WORDS) {
        conditions.push(PatchMarkerAcquired);
        is_mutation_task = true;
    } else if any_word(text, ["search", "find", "locate"]) {
        conditions.push(SearchMarkerAcquired);
        conditions.push(MinBeliefScore(0.65));
    } else if any_word(text, ["read", "inspect", "audit"]) {
        conditions.push(ReadMarkerAcquired);
        conditions.push(MinBeliefScore(0.65));
    } else {
        conditions.push(CommandMarkerAcquired);
        conditions.push(MinBeliefScore(0.85));
    }
}


#### Negation-Aware Word Matching
rust
fn has_word_unnegated(text, word) -> bool {
    // Check 3 preceding tokens for negation markers
    let negations = ["not", "never", "no", "cannot", "n't"];
    // Only counts as trigger if NO negation in window
}

- Fixes misclassification of "Do NOT write implementation" as mutation task
- Used for mutation keywords; simple `any_word` for read/search

#### Contract Evaluation (`evaluate`)
rust
fn evaluate(contract, local_evidence, belief_score) -> bool {
    for condition in contract.conditions {
        match condition {
            MinBeliefScore(min) => belief_score >= min,
            WriteMarkerAcquired => evidence.contains("[WRITE_OK]"),
            PatchMarkerAcquired => evidence.contains("[PATCH_FUNCTION_OK]") || ...,
            ReadMarkerAcquired => evidence.contains("[READ_OK]") || ...,
            SearchMarkerAcquired => evidence.contains("[SEARCH_CODE_OK]") ||