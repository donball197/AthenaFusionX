#!/usr/bin/env python3

from pathlib import Path

ROOT = Path.cwd()

print("=" * 72)
print("ATHENAFUSIONX — PHASE 2 PRE-FLIGHT")
print("=" * 72)
print(f"Repo: {ROOT}")
print()

required = [
    "Cargo.toml",
    "crates/nexus_substrate/src/lib.rs",
    "crates/nexus_substrate/src/memory.rs",
    "crates/runtime_core/src/security/tainted.rs",
]

failed = False

for rel in required:
    p = ROOT / rel
    if p.exists():
        print(f"[OK]   {rel}")
    else:
        print(f"[FAIL] {rel}")
        failed = True

print()
print("=" * 72)
print("TAINT MODULE STATE")
print("=" * 72)

taint = ROOT / "crates/nexus_substrate/src/taint.rs"
lib = (ROOT / "crates/nexus_substrate/src/lib.rs").read_text()
security = (ROOT / "crates/runtime_core/src/security/tainted.rs").read_text()
memory = (ROOT / "crates/nexus_substrate/src/memory.rs").read_text()

if taint.exists():
    print("[WARN] crates/nexus_substrate/src/taint.rs already exists")
else:
    print("[OK]   nexus_substrate::taint does not yet exist")

print(
    "[OK]   runtime_core security TaintLabel remains separate"
    if "pub enum TaintLabel" in security
    else "[FAIL] runtime_core security TaintLabel missing"
)

print(
    "[OK]   pub mod taint; absent — safe registration point"
    if "pub mod taint;" not in lib
    else "[WARN] pub mod taint; already registered"
)

print()
print("=" * 72)
print("AGENT MEMORY STATE")
print("=" * 72)

if "pub taint_label: String" in memory:
    print("[OK]   AgentMemory.taint_label is currently String")
else:
    print("[WARN] AgentMemory.taint_label is no longer the expected String")

constructors = memory.count("map(|row| AgentMemory {")
semantic = memory.count("memories.push(AgentMemory {")

print(f"[OK]   row-based AgentMemory constructors: {constructors}")
print(f"[OK]   semantic AgentMemory constructor:   {semantic}")
print(f"[OK]   total expected constructors:         {constructors + semantic}")

print()
print("=" * 72)
print("PHASE 2 API STATE")
print("=" * 72)

for needle, label in [
    ("parse_taint", "parse_taint"),
    ("to_storage", "to_storage"),
    ("FlowPolicy", "FlowPolicy"),
    ("semantic_search_scoped", "semantic_search_scoped"),
    ("recall_semantic_context_scoped", "recall_semantic_context_scoped"),
]:
    hits = 0
    for base in [
        ROOT / "crates/nexus_substrate/src",
        ROOT / "crates/runtime_core/src",
    ]:
        for p in base.rglob("*.rs"):
            try:
                hits += p.read_text().count(needle)
            except UnicodeDecodeError:
                pass

    print(f"[{'FOUND' if hits else 'ABSENT'}] {label}: {hits} occurrence(s)")

print()
print("=" * 72)
print("DATABASE OWNERSHIP EVIDENCE")
print("=" * 72)

for table in [
    "neural_context",
    "plan_runs",
    "cognition_provenance",
    "state_checkpoints",
]:
    hits = 0
    for p in [
        ROOT / "crates/nexus_substrate/src/memory.rs",
        ROOT / "crates/runtime_core/src/cognition/checkpointer.rs",
        ROOT / "crates/runtime_core/src",
    ]:
        if p.is_file():
            files = [p]
        elif p.exists():
            files = list(p.rglob("*.rs"))
        else:
            files = []

        for f in files:
            try:
                hits += f.read_text().count(table)
            except UnicodeDecodeError:
                pass

    print(f"{table:24} {hits:4} source reference(s)")

print()
print("=" * 72)

if failed:
    print("RESULT: REQUIRED FILE CHECK FAILED")
    raise SystemExit(1)

print("RESULT: PREFLIGHT COMPLETE")
print()
print("No project files were modified by this diagnostic.")
