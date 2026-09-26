#!/usr/bin/env bash
# AthenaFusionX — surgical P1-P5 fix, v2.1
#
# --check: read-only preflight; NOTHING is modified.
# --apply: only runs after a successful preflight; backups go to /tmp.
#
# Deliberately NOT patched here:
#   - derive_strategies_for_node() strategy hook: printed for manual review.
#   - successful send_task() CompleteGoal insertion: printed for manual review.
#
set -euo pipefail

cd ~/AthenaFusionX
PKG="${PKG:-nexus_runtime_core}"

command -v python3 >/dev/null || {
    echo "python3 missing"
    exit 1
}

python3 -c 'import mpmath' 2>/dev/null \
    || echo "WARN: mpmath not importable — verification commands need it"

# ============================================================================
# 0. Anchored patcher
# ============================================================================
cat > /tmp/athena_patch.py <<'PY'
#!/usr/bin/env python3
"""AthenaFusionX P1-P5 v2.1 anchored surgical patcher.

--check is strictly read-only.
--apply is permitted only after every required anchor has passed preflight.
"""
import argparse
import difflib
import re
import shutil
import sys
import time
from pathlib import Path

ROOT = Path.cwd()
SRC = ROOT / "crates" / "runtime_core" / "src"

PLANNER = SRC / "orchestrator_v2" / "planner.rs"
CORTEX = SRC / "executive_cortex.rs"
TASKS = SRC / "task_manager.rs"
DERIVATION = SRC / "derivation_contract.rs"

PATCHES = []
VARIANTS = []


def replace(path, label, anchor, new):
    PATCHES.append(("replace", path, label, anchor, new))


def insert_before(path, label, anchor, new):
    PATCHES.append(("insert_before", path, label, anchor, new))


def ensure_variant(path, label, enum_name, variants):
    VARIANTS.append((path, label, enum_name, variants))


# ============================================================================
# P1 — prompt explicitly separates marker criteria from strategy strings
# ============================================================================
replace(
    PLANNER,
    "P1 prompt: marker-only success criteria",
    "one success criterion from:",
    """one success criterion from the MANDATORY markers below (marker tokens only — never prose
such as "report-only" or "verify numerically", and never strategy strings such as
"execute_command:<cmd>"; the validator accepts nothing but the markers. For a pure
mathematical derivation the correct criterion is CMD_OK, and the verification command
belongs in the node objective):""",
)


# ============================================================================
# P4 + P2/P3 — authoritative contract inference, then derivation fallback
# ============================================================================
replace(
    PLANNER,
    "P4+P2/P3 fallback: inference + derivation branch",
    """        if lower.contains("command") || lower.contains("execute ") || lower.starts_with("run ") {
            return Ok(vec!["CMD_OK".to_string()]);
        }""",
    """        // CMD_OK is a physical command-execution contract. Do not award it merely
        // because the wording mentions "command"/"execute"/"run". The authoritative
        // source is contract inference, shared with the executor.
        let inferred = crate::completion_engine::SubgoalCompletionEngine::infer_contract(
            "planner_fallback",
            directive,
        );

        if crate::derivation_contract::command_required(&inferred.conditions) {
            return Ok(vec!["CMD_OK".to_string()]);
        }

        // P2/P3: a pure mathematical derivation may receive CMD_OK only when
        // derivation_contract can bind the ACTUAL requested mathematics to a
        // grammar-validated verification command. The criterion remains a
        // MARKER; the command belongs in the node objective.
        if crate::derivation_contract::verification_command(directive).is_some() {
            return Ok(vec!["CMD_OK".to_string()]);
        }""",
)


replace(
    PLANNER,
    "P4b reuse inferred contract for read_only",
    """        let read_only = crate::completion_engine::SubgoalCompletionEngine::infer_contract(
            "planner_fallback",
            directive,
        )
        .read_only;""",
    """        let read_only = inferred.read_only;""",
)


# ============================================================================
# Existing hardened mathematical predicate becomes the single policy seam.
#
# This is intentionally a visibility-only change. We do NOT create a second
# mathematical detector in derivation_contract.rs.
# ============================================================================
replace(
    PLANNER,
    "single math-policy seam",
    "fn is_mathematical_derivation_directive(directive: &str) -> bool",
    "pub(crate) fn is_mathematical_derivation_directive(directive: &str) -> bool",
)


# ============================================================================
# Executive lifecycle
# ============================================================================
replace(
    CORTEX,
    "E1 ExecutiveCommand gains ids + terminal ops",
    """pub enum ExecutiveCommand {
    InjectGoal { objective: String, parent_id: Option<Uuid> },
    EvaluateEnvironment,
    EmergencyHalt { reason: String },
}""",
    """pub enum ExecutiveCommand {
    InjectGoal { id: Uuid, objective: String, parent_id: Option<Uuid> },
    CompleteGoal { id: Uuid },
    FailGoal { id: Uuid, reason: String },
    EvaluateEnvironment,
    EmergencyHalt { reason: String },
}""",
)


ensure_variant(
    CORTEX,
    "E1b goal status gets Failed terminal state",
    "ExecutiveGoalStatus",
    ["Failed"],
)


replace(
    CORTEX,
    "E2 InjectGoal adopts caller's id",
    """            ExecutiveCommand::InjectGoal { objective, parent_id } => {
                let id = Uuid::new_v4();""",
    """            ExecutiveCommand::InjectGoal { id, objective, parent_id } => {""",
)


replace(
    CORTEX,
    "E3 CompleteGoal / FailGoal arms",
    """            ExecutiveCommand::EvaluateEnvironment => {
                tracing::info!("[EXECUTIVE CORTEX] Environment evaluation requested.");
            }""",
    """            ExecutiveCommand::CompleteGoal { id } => match self.goals.get_mut(&id) {
                Some(goal) => {
                    goal.status = ExecutiveGoalStatus::Completed;
                    goal.history.push("Goal completed.".to_string());
                    tracing::info!("[EXECUTIVE CORTEX] Goal {} completed.", id);
                }
                None => {
                    tracing::warn!(
                        "[EXECUTIVE CORTEX] CompleteGoal for unknown goal {}",
                        id
                    );
                }
            },
            ExecutiveCommand::FailGoal { id, reason } => match self.goals.get_mut(&id) {
                Some(goal) => {
                    goal.status = ExecutiveGoalStatus::Failed;
                    goal.history.push(format!("Failed: {}", reason));
                    tracing::error!(
                        "[EXECUTIVE CORTEX] Goal {} failed: {}",
                        id,
                        reason
                    );
                }
                None => {
                    tracing::warn!(
                        "[EXECUTIVE CORTEX] FailGoal for unknown goal {}",
                        id
                    );
                }
            },
            ExecutiveCommand::EvaluateEnvironment => {
                tracing::info!("[EXECUTIVE CORTEX] Environment evaluation requested.");
            }""",
)


# ============================================================================
# task_manager.rs
# ============================================================================

replace(
    TASKS,
    "T1 InjectGoal carries the real goal id",
    """    let goal_id = uuid::Uuid::new_v4();
    let _ = command_tx
        .send(ExecutiveCommand::InjectGoal {
            objective: raw_directive.to_string(),
            parent_id: None,
        })
        .await;""",
    """    let goal_id = uuid::Uuid::new_v4();
    let _ = command_tx
        .send(ExecutiveCommand::InjectGoal {
            id: goal_id,
            objective: raw_directive.to_string(),
            parent_id: None,
        })
        .await;""",
)


replace(
    TASKS,
    "T2 planner failure -> fracture()",
    """            Err(e) => return format!("FRACTURE: Planner failed to decompose objective: {}", e),
        };""",
    """            Err(e) => {
                return fracture(
                    &ctx,
                    &command_tx,
                    goal_id,
                    format!("Planner failed to decompose objective: {}", e),
                )
                .await;
            }
        };""",
)


replace(
    TASKS,
    "T3 missing write node -> fracture()",
    """            return format!(
                "FRACTURE: Directive requires writing '{}' but Planner produced no node for it. Refusing to run an incomplete DAG.",
                expected_path.path
            );""",
    """            return fracture(
                &ctx,
                &command_tx,
                goal_id,
                format!(
                    "Directive requires writing '{}' but Planner produced no node for it. Refusing to run an incomplete DAG.",
                    expected_path.path
                ),
            )
            .await;""",
)


replace(
    TASKS,
    "T4 graph validation -> fracture()",
    """        Err(e) => return format!("FRACTURE: Graph validation failed: {}", e),""",
    """        Err(e) => {
            return fracture(
                &ctx,
                &command_tx,
                goal_id,
                format!("Graph validation failed: {}", e),
            )
            .await;
        }""",
)


insert_before(
    TASKS,
    "T5 fracture() helper",
    "pub async fn send_task(",
    """/// P5: explicit planner-phase failure lifecycle.
///
/// Every annotated planner-phase failure:
///   1. sends FailGoal to the Executive Cortex and awaits delivery;
///   2. emits FAILED to the UI with the exact reason;
///   3. returns the same operator-visible FRACTURE string.
async fn fracture(
    ctx: &TaskContext,
    command_tx: &tokio::sync::mpsc::Sender<ExecutiveCommand>,
    goal_id: uuid::Uuid,
    reason: String,
) -> String {
    tracing::error!(%reason, "mission fractured");

    let _ = command_tx
        .send(ExecutiveCommand::FailGoal {
            id: goal_id,
            reason: reason.clone(),
        })
        .await;

    let out = format!("FRACTURE: {}", reason);

    let _ = ctx.tx.send(
        serde_json::json!({
            "type": "DAG_STATE",
            "id": goal_id.to_string(),
            "objective": "MISSION",
            "status": "FAILED",
            "reason": reason,
        })
        .to_string(),
    );

    let _ = ctx.tx.send(
        serde_json::json!({
            "type": "LOG_EVENT",
            "payload": format!("[SCHEDULER] {}", out)
        })
        .to_string(),
    );

    out
}

""",
)


insert_before(
    TASKS,
    "T6 post-plan derivation repair",
    "    if let Some(expected_path) =",
    """    // P2/P3: bind a grammar-validated mathematical verification command
    // into at most one eligible node. The success criterion remains CMD_OK.
    //
    // This is deliberately after plan generation: the planner's graph remains
    // authoritative for decomposition, while this repair only supplies the
    // missing physical command contract for a genuinely pure derivation.
    crate::derivation_contract::repair_derivation_graph(
        &mut graph,
        raw_directive,
    );

""",
)


# ============================================================================
# Module registration — idempotent.
# ============================================================================
def registration_state():
    for cand in (SRC / "lib.rs", SRC / "main.rs"):
        if not cand.exists():
            continue

        text = cand.read_text()

        completion = [
            l for l in text.splitlines()
            if re.search(r"^\s*(?:pub\s+)?mod completion_engine;\s*$", l)
        ]

        if len(completion) == 1:
            derivation = re.search(
                r"^\s*(?:pub\s+)?mod derivation_contract;\s*$",
                text,
                re.M,
            )

            return cand, completion[0], derivation is not None

    return None, None, False


# ============================================================================
# Exact helpers
# ============================================================================
def first_line(s):
    return s.strip().splitlines()[0].strip()


def status_of(kind, text, anchor, new):
    if kind == "replace":
        n = text.count(anchor)
    else:
        n = len([
            line for line in text.splitlines()
            if anchor in line
        ])

    if n == 1:
        return "ok", n

    # Idempotent re-run: the replacement is already present.
    if n == 0 and first_line(new) and first_line(new) in text:
        return "done", 0

    return f"x{n}", n


def all_returns_in_send_task():
    text = TASKS.read_text()
    start = text.index("pub async fn send_task(")
    brace = text.index("{", start)

    # Simple balanced-brace scan is enough for the audit report.
    depth = 0
    end = None

    for i in range(brace, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                end = i + 1
                break

    if end is None:
        raise RuntimeError("could not find end of send_task")

    body = text[start:end]
    base_line = text[:start].count("\n") + 1

    results = []
    for offset, line in enumerate(body.splitlines(), 0):
        if re.search(r"\breturn\b", line):
            results.append((base_line + offset, line))

    return results


def lifecycle_context():
    text = TASKS.read_text()
    start = text.index("pub async fn send_task(")
    lines = text.splitlines()

    start_line = text[:start].count("\n")

    # Print the entire send_task function, but cap it to avoid a giant report.
    brace_line = start_line
    depth = 0
    end_line = None

    for i in range(start_line, len(lines)):
        depth += lines[i].count("{")
        depth -= lines[i].count("}")
        if i > start_line and depth == 0:
            end_line = i
            break

    if end_line is None:
        end_line = min(len(lines) - 1, start_line + 250)

    print("\n--- send_task lifecycle audit ---")

    for i in range(start_line, end_line + 1):
        if (
            "return" in lines[i]
            or "FailGoal" in lines[i]
            or "CompleteGoal" in lines[i]
            or "fracture(" in lines[i]
        ):
            lo = max(start_line, i - 2)
            hi = min(end_line, i + 2)

            print(f"\n  context around line {i + 1}:")
            for j in range(lo, hi + 1):
                print(f"    {j + 1:6}: {lines[j]}")


def preflight():
    ok = True

    print("\n=== REQUIRED PATCH ANCHORS ===")

    for kind, path, label, anchor, new in PATCHES:
        if not path.exists():
            print(f"  MISSING FILE  {label}: {path}")
            ok = False
            continue

        text = path.read_text()
        state, n = status_of(kind, text, anchor, new)

        print(f"  {state:<7} {label}")

        if state.startswith("x"):
            ok = False

            first = anchor.splitlines()[0].strip()

            for line in text.splitlines():
                if first[:50] and first[:50] in line:
                    print(f"          near: {line.strip()[:120]}")

    print("\n=== EXECUTIVE STATUS VARIANTS ===")

    for path, label, enum_name, variants in VARIANTS:
        if not path.exists():
            print(f"  MISSING FILE  {label}: {path}")
            ok = False
            continue

        text = path.read_text()

        m = re.search(
            rf"enum {re.escape(enum_name)}\s*\{{([^}}]*)\}}",
            text,
            re.S,
        )

        if not m:
            print(f"  MISSING ENUM  {label}")
            ok = False
            continue

        missing = [
            v for v in variants
            if not re.search(rf"\b{re.escape(v)}\b", m.group(1))
        ]

        if missing:
            print(
                f"  ok       {label}: will add {missing}"
            )
        else:
            print(
                f"  done     {label}: variants already present"
            )

    cand, line, already = registration_state()

    if not cand:
        print("  MISSING  module registration anchor")
        ok = False
    elif already:
        print(f"  done     module registration already present in {cand}")
    else:
        print(f"  ok       module registration available in {cand}")

    print("\n=== REQUIRED SOURCE SEAM ===")

    planner_text = PLANNER.read_text()

    if "pub(crate) fn is_mathematical_derivation_directive" in planner_text:
        print("  ok       existing hardened math predicate is already crate-visible")
    elif "fn is_mathematical_derivation_directive" in planner_text:
        print("  ok       existing hardened math predicate can be made crate-visible")
    else:
        print("  MISSING  hardened math predicate")
        ok = False

    print("\n=== SEND_TASK RETURN AUDIT ===")

    try:
        returns = all_returns_in_send_task()
    except Exception as exc:
        print(f"  ERROR    {exc}")
        return False

    for line_no, line in returns:
        print(f"  {line_no:6}: {line.strip()}")

    print("\n  MANUAL REQUIREMENT:")
    print("    Every planner-phase failure return must await fracture(...).")
    print("    The normal successful terminal return must await CompleteGoal.")
    print("    No Drop/try_send safety net is used.")

    print("\n=== EXECUTIVE PULSE AUDIT ===")

    cortex_text = CORTEX.read_text()

    if "== ExecutiveGoalStatus::Active" in cortex_text:
        print(
            "  ok       cognitive_tick currently selects only Active goals; "
            "Failed remains terminal"
        )
    else:
        print(
            "  REVIEW   could not find the expected Active-only pulse filter"
        )

    print("\n=== STRATEGY/CRITERION SEAM ===")

    for i, line in enumerate(planner_text.splitlines(), 1):
        if "derive_strategies_for_node" in line:
            print(f"  {i:6}: {line.strip()}")

    print("""
  MANUAL STRATEGY HOOK:
    If derive_strategies_for_node() has a pure-math refusal branch, its
    command-bearing return must use strategy vocabulary:

        Ok(vec![format!("execute_command:{}", cmd)])

    Never place execute_command:<cmd> into success_criteria.
    This patch intentionally does not auto-edit that unseen branch.
""")

    print("\n=== PRE-FLIGHT RESULT ===")
    print("  PASS — no files modified by --check" if ok
          else "  FAIL — nothing may be modified")

    return ok


def report():
    print("\n=== CANDIDATE MATH FUNCTIONS ===")

    for i, line in enumerate(PLANNER.read_text().splitlines(), 1):
        if re.search(
            r"fn .*(math|verification|pure_derivation)",
            line,
        ):
            print(f"  planner.rs:{i}: {line.strip()}")

    print("\n=== DERIVATION MODULE SEAM ===")
    print("  derivation_contract::is_pure_math_mission() delegates to:")
    print("  PlannerAgent::is_mathematical_derivation_directive()")
    print("  No second mathematical detector is retained.")

    print("""
=== MANUAL SUCCESS-PATH LINE ===

Immediately before the NORMAL successful terminal return at the end of
send_task(), add:

    let _ = command_tx
        .send(ExecutiveCommand::CompleteGoal { id: goal_id })
        .await;

Do not add this to an early return. The early returns are handled by fracture().

=== MANUAL STRATEGY LINE ===

If the existing derive_strategies_for_node() refusal path needs the same
mathematical command, the return belongs in the STRATEGY field:

    Ok(vec![format!("execute_command:{}", cmd)])

It does NOT belong in success_criteria.
""")


def apply():
    stamp = time.strftime("%Y%m%d-%H%M%S")
    backup = Path("/tmp") / f"athena-backup-{stamp}"
    backup.mkdir(parents=True, exist_ok=True)

    affected = {p[1] for p in PATCHES} | {
        v[0] for v in VARIANTS
    }

    for path in sorted(affected):
        if not path.exists():
            continue

        dst = backup / path.relative_to(ROOT)
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, dst)

    print(f"\n=== APPLYING PATCHES ===")
    print(f"backup: {backup}")

    for kind, path, label, anchor, new in PATCHES:
        text = path.read_text()
        state, n = status_of(kind, text, anchor, new)

        if state == "done":
            print(f"  skip    {label} (already applied)")
            continue

        if state != "ok":
            raise RuntimeError(
                f"anchor state changed during apply: {label}: {state}"
            )

        if kind == "replace":
            after = text.replace(anchor, new, 1)
        else:
            lines = text.splitlines(keepends=True)
            matches = [
                i for i, line in enumerate(lines)
                if anchor in line
            ]

            if len(matches) != 1:
                raise RuntimeError(
                    f"insert anchor changed during apply: {label}"
                )

            i = matches[0]
            block = new if new.endswith("\n") else new + "\n"
            lines[i:i] = [block]
            after = "".join(lines)

        path.write_text(after)

        print(f"  applied {label}")

        for line in difflib.unified_diff(
            text.splitlines(),
            after.splitlines(),
            f"a/{path.relative_to(ROOT)}",
            f"b/{path.relative_to(ROOT)}",
            lineterm="",
        ):
            print("   ", line)

    # ------------------------------------------------------------------------
    # Add Failed variant only when absent.
    # ------------------------------------------------------------------------
    for path, label, enum_name, variants in VARIANTS:
        text = path.read_text()

        m = re.search(
            rf"(enum {re.escape(enum_name)}\s*\{{)([^}}]*)(\}})",
            text,
            re.S,
        )

        if not m:
            raise RuntimeError(
                f"cannot locate enum during apply: {label}"
            )

        missing = [
            v for v in variants
            if not re.search(
                rf"\b{re.escape(v)}\b",
                m.group(2),
            )
        ]

        if not missing:
            print(f"  skip    {label} (already present)")
            continue

        body = m.group(2).rstrip()

        if body and not body.endswith(","):
            body += ","

        new_body = body + "".join(
            f"\n    {v},"
            for v in missing
        ) + "\n"

        after = (
            text[:m.start(2)]
            + new_body
            + text[m.end(2):]
        )

        path.write_text(after)
        print(f"  applied {label}: added {missing}")

    # ------------------------------------------------------------------------
    # Idempotent module registration.
    # ------------------------------------------------------------------------
    cand, line, already = registration_state()

    if already:
        print("  skip    module registration (already present)")
    elif cand:
        text = cand.read_text()

        prefix = (
            "pub "
            if line.strip().startswith("pub ")
            else ""
        )

        lines = text.splitlines(keepends=True)

        i = next(
            j for j, l in enumerate(lines)
            if re.search(
                r"^\s*(?:pub\s+)?mod completion_engine;\s*$",
                l,
            )
        )

        lines[i + 1:i + 1] = [
            f"{prefix}mod derivation_contract;\n"
        ]

        cand.write_text("".join(lines))
        print(f"  applied module registration in {cand}")
    else:
        raise RuntimeError(
            "module registration anchor disappeared during apply"
        )

    print(f"\nbackup: {backup}")


parser = argparse.ArgumentParser()
parser.add_argument("--check", action="store_true")
parser.add_argument("--apply", action="store_true")
args = parser.parse_args()

if args.check and args.apply:
    parser.error("use --check OR --apply, not both")

if not args.check and not args.apply:
    parser.error("use --check or --apply")

print("============================================================")
print("AthenaFusionX P1-P5 v2.1")
print("============================================================")
print()

good = preflight()
report()

if args.apply:
    if not good:
        sys.exit("preflight failed — NOTHING modified")

    apply()
    print("\n[PATCH_APPLY_OK] P1-P5 v2.1 source edits applied.")
elif args.check:
    sys.exit(0 if good else "preflight failed — NOTHING modified")
PY

# ============================================================================
# 1. READ-ONLY PREFLIGHT
#
# IMPORTANT:
#   The new Rust module is NOT written until this succeeds.
# ============================================================================
python3 /tmp/athena_patch.py --check

# ============================================================================
# 2. ONLY AFTER PREFLIGHT PASSES: create derivation_contract.rs
# ============================================================================
cat > crates/runtime_core/src/derivation_contract.rs <<'RUST'
//! Deterministic contract helpers for pure mathematical-derivation missions.
//!
//! Vocabulary contract:
//!   * success criteria are MARKERS: CMD_OK, READ_OK:<path>, ...
//!   * strategies are imperative strings: execute_command:<cmd>
//!
//! The mathematical mission predicate is deliberately delegated to the
//! existing hardened planner predicate. This module does not create a second
//! mathematical-policy detector.

use crate::completion_engine::{
    CompletionCondition,
    SubgoalCompletionEngine,
};

/// Marker vocabulary accepted by success-contract validation.
pub const CMD_OK: &str = "CMD_OK";

/// True when the inferred completion contract requires physical command
/// execution.
pub fn command_required(conditions: &[CompletionCondition]) -> bool {
    conditions.iter().any(|condition| {
        matches!(
            condition,
            CompletionCondition::CommandExecutionVerified { .. }
                | CompletionCondition::CommandMarkerAcquired
        )
    })
}

/// Return the exact command carried by the authoritative completion contract,
/// when one exists.
pub fn extract_required_command(
    conditions: &[CompletionCondition],
) -> Option<String> {
    conditions.iter().find_map(|condition| match condition {
        CompletionCondition::CommandExecutionVerified { command } => {
            Some(command.clone())
        }
        _ => None,
    })
}

/// Single mathematical-policy seam.
///
/// The planner owns the hardened predicate. This module only delegates to it;
/// there must not be a second independent mathematical detector.
pub fn is_pure_math_mission(directive: &str) -> bool {
    crate::orchestrator_v2::planner::PlannerAgent::
        is_mathematical_derivation_directive(directive)
}

/// Return a verification command only when the command can be bound to the
/// actual mathematics present in the directive.
///
/// No hardcoded mathematical constant is substituted for a missing identity.
/// No arbitrary prose is copied into Python. Unsupported mathematics returns
/// None and therefore remains on the deterministic refusal path.
pub fn verification_command(directive: &str) -> Option<String> {
    if !is_pure_math_mission(directive) {
        return None;
    }

    // Operator-authored verification recipe.
    if let Some(command) = explicit_recipe(directive) {
        return Some(command);
    }

    // Strict plain-text identity.
    let (lhs, rhs) = parse_identity(directive)?;

    Some(format!(
        "python3 -c \"import mpmath as mp; mp.mp.dps=32; zeta=mp.zeta; pi=mp.pi; phi=mp.phi; gamma=mp.gamma; e=mp.e; sqrt=mp.sqrt; exp=mp.exp; log=mp.log; ln=mp.log; sin=mp.sin; cos=mp.cos; tan=mp.tan; asin=mp.asin; acos=mp.acos; atan=mp.atan; sinh=mp.sinh; cosh=mp.cosh; tanh=mp.tanh; factorial=mp.factorial; mpf=mp.mpf; print('abs(lhs-rhs) =', abs(({lhs}) - ({rhs})))\""
    ))
}

/// An explicit interpreter command supplied by the directive itself.
///
/// This deliberately accepts only a command span whose beginning is an
/// explicitly recognized interpreter invocation. Arbitrary backtick prose is
/// never promoted to executable evidence.
fn explicit_recipe(directive: &str) -> Option<String> {
    for span in directive
        .split('`')
        .skip(1)
        .step_by(2)
    {
        let command = span.trim();

        if command.contains('\n') {
            continue;
        }

        let head = command
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");

        if [
            "python3 -c",
            "python -c",
            "sage -c",
            "julia -e",
        ]
        .contains(&head.as_str())
        {
            return Some(command.to_string());
        }

        if [
            "python3",
            "sage",
            "julia",
        ]
        .contains(&head.as_str())
        {
            return Some(command.to_string());
        }
    }

    None
}

/// Find a plain-text `lhs = rhs` identity.
///
/// This intentionally does NOT attempt to parse LaTeX. Unsupported LaTeX
/// remains a deterministic refusal rather than becoming fabricated evidence.
fn parse_identity(directive: &str) -> Option<(String, String)> {
    for raw in directive.lines() {
        let line = raw
            .trim()
            .trim_end_matches('.')
            .trim();

        let line = line
            .rsplit_once(':')
            .map(|(_, rhs)| rhs.trim())
            .unwrap_or(line);

        if let Some((lhs, rhs)) = line.split_once('=') {
            let lhs = lhs
                .trim()
                .trim_end_matches('.')
                .trim();

            let rhs = rhs
                .trim()
                .trim_end_matches('.')
                .trim_end_matches(',')
                .trim();

            if !lhs.is_empty()
                && !rhs.is_empty()
                && safe_expr(lhs)
                && safe_expr(rhs)
            {
                return Some((
                    lhs.to_string(),
                    rhs.to_string(),
                ));
            }
        }
    }

    None
}

/// Strict expression grammar.
///
/// `^` is intentionally NOT accepted. Python uses `^` for bitwise XOR, so
/// silently accepting mathematical caret notation would change the meaning
/// of the requested mathematics. A future explicit normalization contract can
/// add caret-to-power conversion with dedicated tests.
const ALLOWED_NAMES: [&str; 27] = [
    "pi",
    "e",
    "phi",
    "zeta",
    "gamma",
    "sqrt",
    "exp",
    "log",
    "ln",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "sinh",
    "cosh",
    "tanh",
    "abs",
    "max",
    "min",
    "pow",
    "floor",
    "ceil",
    "factorial",
    "mp",
    "mpf",
];

fn safe_expr(s: &str) -> bool {
    let charset_ok = s.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || "+-*/().,_ ".contains(c)
    });

    if !charset_ok {
        return false;
    }

    let mut value_seen = false;

    for token in s.split(|c: char| {
        !(c.is_ascii_alphanumeric() || c == '_' || c == '.')
    }) {
        if token.is_empty() {
            continue;
        }

        if token
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        {
            value_seen = true;
            continue;
        }

        if !token
            .split('.')
            .all(|part| ALLOWED_NAMES.contains(&part))
        {
            return false;
        }

        value_seen = true;
    }

    value_seen
}

/// Post-plan repair.
///
/// At most one node is changed. The deterministic target is the lowest node
/// identifier whose objective is not itself an artifact-writing objective.
///
/// The criterion is always CMD_OK. The command is embedded in the objective,
/// allowing authoritative contract inference to identify the command.
pub fn repair_derivation_graph(
    graph: &mut crate::orchestrator_v2::task_graph::TaskGraph,
    directive: &str,
) {
    let command = match verification_command(directive) {
        Some(command) => command,
        None => return,
    };

    let already_verifiable = graph.nodes.values().any(|node| {
        command_required(
            &SubgoalCompletionEngine::infer_contract(
                &node.id,
                &node.objective,
            )
            .conditions,
        )
    });

    if already_verifiable {
        return;
    }

    let artifact_cues = [
        "write ",
        "save ",
        "export ",
        "create ",
        "patch ",
        "write_ok",
        ".md",
        ".rs",
        ".py",
        ".txt",
        ".toml",
    ];

    let target = graph
        .nodes
        .iter()
        .filter(|(_, node)| {
            let lower = node.objective.to_lowercase();

            !artifact_cues
                .iter()
                .any(|cue| lower.contains(cue))
        })
        .map(|(id, _)| id.clone())
        .min();

    let target = match target {
        Some(id) => id,
        None => {
            tracing::warn!(
                "[DERIVATION REPAIR] no eligible node for verification contract"
            );
            return;
        }
    };

    if let Some(node) = graph.nodes.get_mut(&target) {
        let original = node.objective.clone();

        node.objective = format!(
            "Execute `{}` and report the actual command used and its output as verifiable evidence.\n\nMANDATORY NODE SUCCESS CRITERIA:\nCMD_OK\n\nThen complete the original objective:\n{}",
            command,
            original
        );

        node.success_criteria = vec![
            CMD_OK.to_string()
        ];

        tracing::info!(
            "[DERIVATION REPAIR] node {} carries verification command contract",
            target
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegation_uses_existing_math_policy() {
        let mission =
            "Derive the mathematical identity and verify numerically: pi = 22/7.";

        assert_eq!(
            is_pure_math_mission(mission),
            crate::orchestrator_v2::planner::PlannerAgent::
                is_mathematical_derivation_directive(mission)
        );
    }

    #[test]
    fn artifact_request_is_not_pure_math() {
        assert!(!is_pure_math_mission(
            "Derive the result and write model_routing_analysis.md."
        ));
    }

    #[test]
    fn identity_gets_real_command_when_policy_accepts_it() {
        let mission =
            "Derive the mathematical identity and verify numerically: pi = 22/7.";

        if is_pure_math_mission(mission) {
            let command = verification_command(mission)
                .expect("plain identity should parse");

            assert!(command.starts_with("python3 -c "));
            assert!(command.contains("abs((pi) - (22/7))"));
        }
    }

    #[test]
    fn unsupported_latex_fabricates_nothing() {
        assert_eq!(
            verification_command(
                r"Derive the mathematical identity and verify numerically: \frac{1}{2} = 0.5."
            ),
            None
        );
    }

    #[test]
    fn caret_is_rejected() {
        assert!(!safe_expr("pi^2"));
    }

    #[test]
    fn unsafe_expression_text_is_rejected() {
        assert!(!safe_expr("__import__(\"os\").system(\"id\")"));
        assert!(!safe_expr("1+1; print(2)"));
        assert!(!safe_expr("open('x')"));
        assert!(safe_expr("zeta(3) / pi"));
    }

    #[test]
    fn explicit_recipe_is_used_verbatim() {
        let mission =
            "Verify numerically. Command: `python3 -c \"print(1)\"`.";

        if is_pure_math_mission(mission) {
            assert_eq!(
                verification_command(mission).as_deref(),
                Some("python3 -c \"print(1)\"")
            );
        }
    }

    #[test]
    fn command_required_matches_execution_contracts() {
        assert!(
            command_required(
                &[CompletionCondition::CommandExecutionVerified {
                    command: "uptime".to_string()
                }]
            )
        );

        assert!(
            !command_required(
                &[CompletionCondition::ReadPathAcquired {
                    path: "a.rs".to_string()
                }]
            )
        );
    }
}
RUST

# ============================================================================
# 3. APPLY — preflight has already passed
# ============================================================================
python3 /tmp/athena_patch.py --apply

# ============================================================================
# 4. FORMAT ONLY THE THREE TOUCHED RUST AREAS
# ============================================================================
cargo fmt -- \
    crates/runtime_core/src/orchestrator_v2/planner.rs \
    crates/runtime_core/src/executive_cortex.rs \
    crates/runtime_core/src/task_manager.rs \
    crates/runtime_core/src/derivation_contract.rs

# ============================================================================
# 5. DIFF CHECK — scoped to the files this repair owns
# ============================================================================
git diff --check -- \
    crates/runtime_core/src/orchestrator_v2/planner.rs \
    crates/runtime_core/src/executive_cortex.rs \
    crates/runtime_core/src/task_manager.rs \
    crates/runtime_core/src/derivation_contract.rs

# ============================================================================
# 6. COMPILE
# ============================================================================
cargo check -p "$PKG" --message-format=short 2>&1 | tail -40
