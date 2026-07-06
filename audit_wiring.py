#!/usr/bin/env python3
"""
audit_wiring.py
Read-only diagnostic — makes no changes to any file.

Checks two things:
1. Module-level reachability: for every `pub mod X;` declared in
   crates/runtime_core/src/lib.rs, does anything OUTSIDE that module's own
   file(s) ever reference `X::`? If not, it's compiling dead weight —
   present in the binary, invisible to cargo check/clippy, never executed.
2. Known-suspect spot-check: specific functions we've already caught this
   session as "wired enough to compile, not wired enough to run" — checked
   by exact call-site pattern, not just module name, since a module can be
   referenced once (e.g. by task_manager_v4) while still being a dead end
   itself.

Run from the AthenaFusionX repo root:
    python3 audit_wiring.py
"""
import os
import re
import sys

REPO_ROOT = "."
LIB_RS = "crates/runtime_core/src/lib.rs"
SRC_ROOT = "crates/runtime_core/src"

EXCLUDE_DIRS = {"target", "vendor", "third_party", "node_modules", ".git"}


def find_rs_files():
    out = []
    for dirpath, dirnames, filenames in os.walk(REPO_ROOT):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDE_DIRS]
        for fn in filenames:
            if fn.endswith(".rs"):
                out.append(os.path.normpath(os.path.join(dirpath, fn)))
    return out


def module_files(mod_name):
    """Return the set of files that belong to this module (mod.rs style
    submodules count as part of the parent module's own file set)."""
    candidates = []
    single = os.path.join(SRC_ROOT, f"{mod_name}.rs")
    dir_mod = os.path.join(SRC_ROOT, mod_name, "mod.rs")
    if os.path.isfile(single):
        candidates.append(os.path.normpath(single))
    if os.path.isdir(os.path.join(SRC_ROOT, mod_name)):
        for dirpath, _, filenames in os.walk(os.path.join(SRC_ROOT, mod_name)):
            for fn in filenames:
                if fn.endswith(".rs"):
                    candidates.append(os.path.normpath(os.path.join(dirpath, fn)))
    return set(candidates)


def main():
    if not os.path.isfile(LIB_RS):
        print(f"FATAL: {LIB_RS} not found. Run this from the AthenaFusionX repo root.", file=sys.stderr)
        sys.exit(1)

    with open(LIB_RS) as f:
        lib_content = f.read()

    declared_modules = re.findall(r"^pub mod (\w+);", lib_content, re.MULTILINE)

    all_files = find_rs_files()
    file_contents = {}
    for fp in all_files:
        try:
            with open(fp, errors="ignore") as f:
                file_contents[fp] = f.read()
        except Exception:
            pass

    print("=" * 70)
    print("PART 1: Module-level reachability")
    print(f"({len(declared_modules)} modules declared in lib.rs)")
    print("=" * 70)

    orphaned = []
    weakly_referenced = []

    for mod in sorted(declared_modules):
        own_files = module_files(mod)
        pattern = re.compile(rf"\b{re.escape(mod)}::")
        external_hits = []
        for fp, content in file_contents.items():
            if fp in own_files:
                continue
            matches = pattern.findall(content)
            if matches:
                external_hits.append((fp, len(matches)))

        total_refs = sum(c for _, c in external_hits)
        distinct_files = len(external_hits)

        if total_refs == 0:
            orphaned.append(mod)
            print(f"  [ORPHANED]   {mod:<20} — 0 external references anywhere in the workspace")
        elif distinct_files == 1 and total_refs <= 2:
            weakly_referenced.append((mod, external_hits))
            loc = external_hits[0][0]
            print(f"  [WEAK]       {mod:<20} — only {total_refs} reference(s), only from {loc}")
        else:
            print(f"  [wired]      {mod:<20} — {total_refs} references across {distinct_files} file(s)")

    print("")
    print("=" * 70)
    print("PART 2: Known-suspect spot-check")
    print("(specific calls, not just module names — a module can be")
    print("'referenced' while its actual entry point is never invoked)")
    print("=" * 70)

    suspects = [
        ("CorticalLoop::ignite(", "crates/runtime_core/src/cortical_loop.rs",
         "Full Strategist/Critic/Translator/Evaluator epistemic loop"),
        ("CorticalLoop::new(", "crates/runtime_core/src/cortical_loop.rs",
         "Constructor for the above"),
        ("MetaReasoner::execute_with_got(", "crates/runtime_core/src/cognition/meta_reasoner.rs",
         "Currently a stub returning a fixed string regardless of input"),
        ("send_task_v4(", "crates/runtime_core/src/task_manager_v4.rs",
         "v4 task manager entry point"),
        ("ReActAgent::new(", "crates/runtime_core/src/react_agent.rs",
         "ReAct-loop agent (uses the ToolExecutor we added)"),
        ("HypothesisEngine::evaluate_fault(", "crates/runtime_core/src/hypothesis_engine.rs",
         "Graph-of-Thoughts fault recovery"),
        ("EventBus::new(", "crates/runtime_core/src/event_bus.rs",
         "Event bus construction"),
        ("ExecutionSandbox::strict_lockdown(", "crates/runtime_core/src/policy.rs",
         "Capability-based sandbox"),
        ("GovernanceLog::init_shared(", "crates/runtime_core/src/governance/protocol.rs",
         "Merkle audit chain (should show up now if you ran the governance wiring script)"),
    ]

    for call_pattern, def_file, description in suspects:
        call_name = call_pattern.rstrip("(")
        hits = []
        for fp, content in file_contents.items():
            if os.path.normpath(fp) == os.path.normpath(def_file):
                continue
            if call_pattern in content:
                hits.append(fp)
        status = "CALLED" if hits else "NEVER CALLED"
        marker = "  [ok]       " if hits else "  [DEAD END] "
        print(f"{marker}{call_name:<32} {status}")
        print(f"               {description}")
        if hits:
            print(f"               called from: {', '.join(hits[:5])}" + (" ..." if len(hits) > 5 else ""))
        print("")

    print("=" * 70)
    print("SUMMARY")
    print("=" * 70)
    print(f"Fully orphaned modules (0 external refs): {len(orphaned)}")
    if orphaned:
        print(f"  {', '.join(orphaned)}")
    print(f"Weakly-referenced modules (<=2 refs, 1 file): {len(weakly_referenced)}")
    if weakly_referenced:
        print(f"  {', '.join(m for m, _ in weakly_referenced)}")
    dead_suspects = [c.rstrip('(') for c, f, d in suspects
                     if not any(c in content for fp, content in file_contents.items()
                                if os.path.normpath(fp) != os.path.normpath(f))]
    print(f"Known-suspect dead ends confirmed: {len(dead_suspects)}")
    if dead_suspects:
        print(f"  {', '.join(dead_suspects)}")
    print("")
    print("NOTE: this is a textual heuristic (substring/regex matching), not a")
    print("real call-graph analysis — it can't see through trait objects, dynamic")
    print("dispatch, or macro-generated code, and a module used only in #[cfg(test)]")
    print("will show as 'wired' even if unreachable from any binary. Treat this as")
    print("a prioritized list to manually verify, not a final verdict.")


if __name__ == "__main__":
    main()
