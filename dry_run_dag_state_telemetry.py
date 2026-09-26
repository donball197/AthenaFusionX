#!/usr/bin/env python3
"""
DAG_STATE telemetry patch — dry-run by default.

Usage:
    python3 dry_run_dag_state_telemetry.py             # show diffs
    python3 dry_run_dag_state_telemetry.py --check     # anchors only
    python3 dry_run_dag_state_telemetry.py --apply     # write, with backups

Three anchors in crates/runtime_core/src/task_manager.rs:

  dispatch   -> emit {"type":"DAG_STATE","status":"RUNNING"}
  completed  -> emit {"type":"DAG_STATE","status":"COMPLETED"}
  failed     -> emit {"type":"DAG_STATE","status":"FAILED"}

Each anchor must match exactly once. If any anchor matches zero or
multiple times, the script aborts before writing anything.
"""
from __future__ import annotations
import argparse
import difflib
import shutil
import sys
import time
from pathlib import Path
from typing import Optional, Tuple

REPO = Path.cwd()
BACKUP_ROOT = REPO / ".taint-patch-backups"
TARGET_REL = "crates/runtime_core/src/task_manager.rs"


# ----------------------------------------------------------------------
# Anchors
# ----------------------------------------------------------------------

DISPATCH_OLD = '''        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[SCHEDULER] Dispatching DAG Node: {}\\nvia Subagent: {}", node.objective, node.required_agent)}).to_string());

        let mut task_context_history = String::new();'''

DISPATCH_NEW = '''        let _ = ctx.tx.send(serde_json::json!({"type": "LOG_EVENT", "payload": format!("[SCHEDULER] Dispatching DAG Node: {}\\nvia Subagent: {}", node.objective, node.required_agent)}).to_string());

        // Structured DAG telemetry: this is the actual production execution boundary.
        let _ = ctx.tx.send(
            serde_json::json!({
                "type": "DAG_STATE",
                "id": task_id,
                "objective": node.objective,
                "status": "RUNNING"
            })
            .to_string(),
        );

        let mut task_context_history = String::new();'''


COMPLETED_OLD = '''                if let Some(graph_node) = graph.nodes.get_mut(&task_id) {
                    graph_node.status = crate::orchestrator_v2::task_graph::TaskStatus::Completed;
                    graph_node.output_evidence = Some(execution_evidence.clone());
                }

                let _ = ctx.tx.send('''

COMPLETED_NEW = '''                if let Some(graph_node) = graph.nodes.get_mut(&task_id) {
                    graph_node.status = crate::orchestrator_v2::task_graph::TaskStatus::Completed;
                    graph_node.output_evidence = Some(execution_evidence.clone());
                }

                // Structured DAG telemetry mirrors the authoritative production mutation.
                let _ = ctx.tx.send(
                    serde_json::json!({
                        "type": "DAG_STATE",
                        "id": task_id,
                        "objective": node.objective,
                        "status": "COMPLETED"
                    })
                    .to_string(),
                );

                let _ = ctx.tx.send('''


FAILED_OLD = '''                if let Some(graph_node) = graph.nodes.get_mut(&task_id) {
                    graph_node.status = crate::orchestrator_v2::task_graph::TaskStatus::Failed;
                    graph_node.output_evidence = Some(format!(
                        "{}\\n[NODE VERIFICATION FAILED] {}",
                        execution_evidence, reason
                    ));
                }

                let _ = ctx.tx.send('''

FAILED_NEW = '''                if let Some(graph_node) = graph.nodes.get_mut(&task_id) {
                    graph_node.status = crate::orchestrator_v2::task_graph::TaskStatus::Failed;
                    graph_node.output_evidence = Some(format!(
                        "{}\\n[NODE VERIFICATION FAILED] {}",
                        execution_evidence, reason
                    ));
                }

                // Structured DAG telemetry mirrors the authoritative production mutation.
                let _ = ctx.tx.send(
                    serde_json::json!({
                        "type": "DAG_STATE",
                        "id": task_id,
                        "objective": node.objective,
                        "status": "FAILED"
                    })
                    .to_string(),
                );

                let _ = ctx.tx.send('''


PATCHES = [
    ("dispatch  -> RUNNING",   DISPATCH_OLD,  DISPATCH_NEW),
    ("completed -> COMPLETED", COMPLETED_OLD, COMPLETED_NEW),
    ("failed    -> FAILED",    FAILED_OLD,    FAILED_NEW),
]


# ----------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------

def show_diff(path: str, a: str, b: str) -> None:
    d = list(difflib.unified_diff(
        a.splitlines(True), b.splitlines(True),
        fromfile=path, tofile=path, n=3,
    ))
    if not d:
        print("  (no textual change)")
        return
    for i, line in enumerate(d):
        if i >= 70:
            print(f"  ... ({len(d) - 70} more diff lines)")
            break
        sys.stdout.write("  " + (line if line.endswith("\n") else line + "\n"))


def check_anchors(source: str) -> Tuple[bool, list[str]]:
    problems: list[str] = []
    for label, old, _ in PATCHES:
        n = source.count(old)
        if n == 0:
            problems.append(f"{label}: anchor not found")
        elif n > 1:
            problems.append(f"{label}: anchor matched {n} times (expected exactly 1)")
    if source.count('"type": "DAG_STATE"') != 0:
        problems.append('DAG_STATE telemetry already present (idempotency guard)')
    return (len(problems) == 0), problems


def transform(source: str) -> str:
    updated = source
    for _, old, new in PATCHES:
        updated = updated.replace(old, new, 1)
    return updated


# ----------------------------------------------------------------------
# Main
# ----------------------------------------------------------------------

def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--apply", action="store_true", help="write the patch")
    g.add_argument("--check", action="store_true", help="verify anchors only")
    args = ap.parse_args()

    target = REPO / TARGET_REL
    if not target.exists():
        print(f"[FAIL] Missing target: {target}", file=sys.stderr)
        return 2

    source = target.read_text(encoding="utf-8")
    ok, problems = check_anchors(source)

    print("=" * 68)
    print("DAG_STATE telemetry patch")
    print("=" * 68)
    print(f"Repo:   {REPO}")
    print(f"Target: {TARGET_REL}")
    print(f"Mode:   {'APPLY' if args.apply else ('CHECK' if args.check else 'DRY RUN')}")
    print("=" * 68)
    print()

    # --- anchor status -------------------------------------------------

    for label, old, _ in PATCHES:
        n = source.count(old)
        mark = "OK  " if n == 1 else "FAIL"
        print(f"  {mark}  anchor [{label}]  ({n} match{'es' if n != 1 else ''})")

    dag_state_hits = source.count('"type": "DAG_STATE"')
    mark = "OK  " if dag_state_hits == 0 else "WARN"
    print(f"  {mark}  existing DAG_STATE telemetry present: {dag_state_hits}")

    print()

    if not ok:
        print("ABORT: anchor problems")
        for p in problems:
            print(f"  - {p}")
        return 1

    # --- check mode ----------------------------------------------------

    if args.check:
        print("All anchors present and idempotency guard clear.")
        print("Re-run without --check for diffs, or with --apply to write.")
        return 0

    # --- dry run -------------------------------------------------------

    if not args.apply:
        print("-" * 68)
        print("Proposed diff")
        print("-" * 68)
        show_diff(TARGET_REL, source, transform(source))
        print()
        print("-" * 68)
        print("Nothing written. Re-run with --apply to commit.")
        print("-" * 68)
        return 0

    # --- apply ---------------------------------------------------------

    BACKUP_ROOT.mkdir(parents=True, exist_ok=True)
    ts = time.strftime("%Y%m%d-%H%M%S")
    bdir = BACKUP_ROOT / f"dag-state-{ts}"
    bdir.mkdir(parents=True, exist_ok=True)

    backup_path = bdir / Path(TARGET_REL).name
    shutil.copy2(target, backup_path)
    print(f"[BACKUP] {backup_path.relative_to(REPO)}")

    updated = transform(source)
    target.write_text(updated, encoding="utf-8")

    print("[PATCH_OK] DAG_STATE telemetry applied")
    print()
    print("Production transitions exposed:")
    print("  dispatch            -> RUNNING")
    print("  verification OK     -> COMPLETED")
    print("  verification other  -> FAILED")
    print()
    print("Dormant orchestrator_v2::scheduler transitions were not modified.")
    print()
    print("Verify:")
    print("  cargo check -p nexus_runtime_core")
    return 0


if __name__ == "__main__":
    sys.exit(main())
