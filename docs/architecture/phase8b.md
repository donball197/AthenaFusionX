# Phase 8B v2 – Hardened Executive Cortex

## Changes from 8B v1

| Item                    | v1                    | v2                                      |
|-------------------------|-----------------------|-----------------------------------------|
| Script safety           | `cat >` overwrite     | `write_if_stub` (idempotent)            |
| EventBus                | sync listeners        | `tokio::broadcast` + EventStore trait   |
| Goal storage            | `Vec<Goal>`           | `HashMap<Uuid, Goal>`                   |
| Blackboard              | basic                 | + `decision_history`                    |
| Executive               | health-only           | staged pipeline (observe→…→emit)        |
| Policy                  | string denylist       | capability-based                        |
| Attention               | single focus          | + salience map                          |
| WorkingMemory           | flat KV               | partitioned (facts/obs/reasoning/hyp)   |

## Still behind `cognition_v5`

CorticalLoop is untouched. These components are available for experimentation and will be wired into the main path in Phase 8C/8D.

## Verify

```bash
cargo check -p nexus_runtime_core --features cognition_v5
cargo clippy -p nexus_runtime_core --features cognition_v5 -- -D warnings
