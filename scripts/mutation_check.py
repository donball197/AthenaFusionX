#!/usr/bin/env python3
"""Mutation check: do the tests actually detect bugs in the module under test?

usage:
  scripts/mutation_check.py MODULE.py TEST_FILE.py [--cmd CMD] [--cwd DIR]
                            [--max-mutants N] [--min-score F] [--timeout S]

CMD is run with bash -c in --cwd; {tests} expands to TEST_FILE. Default:
  python -m pytest {tests} -x -q -p no:cacheprovider

Flow: baseline run must pass -> apply one AST mutation at a time to MODULE.py
(in place, restored in finally/on signal) -> rerun. Non-zero exit or timeout =
killed; exit 0 = SURVIVED (tests are blind to that bug).

Output lines (greppable, for the conformance gate):
  [MUTATION_SURVIVED] file:line  description
  [MUTATION_SCORE] killed/total = 0.xx
Exit: 0 score >= min-score, 1 below, 2 baseline failed / nothing to mutate.
"""
from __future__ import annotations

import argparse
import ast
import copy
import os
import random
import shutil
import signal
import subprocess
import sys
import tempfile

CMP_SWAP = {ast.Eq: ast.NotEq, ast.NotEq: ast.Eq, ast.Lt: ast.LtE, ast.LtE: ast.Lt,
            ast.Gt: ast.GtE, ast.GtE: ast.Gt}
SKIP_FUNCS = {"__repr__", "__str__"}


def _is_docstring(node, parent):
    return (isinstance(node, ast.Constant) and isinstance(node.value, str)
            and isinstance(parent, ast.Expr))


def _in_raise_or_assert(stack):
    return any(isinstance(n, (ast.Raise, ast.Assert)) for n in stack)


def enumerate_mutations(tree: ast.Module):
    """Return list of (id, lineno, description, apply(tree_copy_node_lookup))."""
    muts = []
    nodes = []  # preorder list so node index is stable across deepcopy

    def walk(node, stack):
        nodes.append((node, list(stack)))
        for child in ast.iter_child_nodes(node):
            walk(child, stack + [node])

    walk(tree, [])

    for idx, (node, stack) in enumerate(nodes):
        line = getattr(node, "lineno", 0)
        if any(isinstance(n, ast.FunctionDef) and n.name in SKIP_FUNCS for n in stack):
            continue
        if _in_raise_or_assert(stack) or any(isinstance(n, ast.AnnAssign) and n.value is node for n in stack[-1:]):
            continue
        parent = stack[-1] if stack else None

        if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Sub)):
            sym = "+" if isinstance(node.op, ast.Add) else "-"
            other = ast.Sub if sym == "+" else ast.Add
            muts.append((idx, line, f"swap `{sym}` for `{'-' if sym == '+' else '+'}`",
                         lambda n, o=other: setattr(n, "op", o())))
            muts.append((idx, line, f"drop right operand of `{sym}`",
                         lambda n: _replace_with(n, n.left)))
        elif isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Mult, ast.Div)):
            sym = "*" if isinstance(node.op, ast.Mult) else "/"
            other = ast.Div if sym == "*" else ast.Mult
            muts.append((idx, line, f"swap `{sym}` for `{'/' if sym == '*' else '*'}`",
                         lambda n, o=other: setattr(n, "op", o())))
        elif isinstance(node, ast.Constant) and isinstance(node.value, bool):
            muts.append((idx, line, f"flip bool {node.value} -> {not node.value}",
                         lambda n: setattr(n, "value", not n.value)))
        elif (isinstance(node, ast.Constant) and type(node.value) is int
              and not _is_docstring(node, parent) and node.value in (0, 1, 2, -1)):
            new = {0: 1, 1: 0, 2: 1, -1: 0}[node.value]
            muts.append((idx, line, f"constant {node.value} -> {new}",
                         lambda n, v=new: setattr(n, "value", v)))
        elif isinstance(node, ast.Compare) and type(node.ops[0]) in CMP_SWAP:
            old = type(node.ops[0])
            muts.append((idx, line, f"comparison {old.__name__} -> {CMP_SWAP[old].__name__}",
                         lambda n, o=CMP_SWAP[old]: n.ops.__setitem__(0, o())))
        elif (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
              and node.func.attr in ("permute", "transpose") and len(node.args) >= 2):
            muts.append((idx, line, f"swap first two args of .{node.func.attr}()",
                         lambda n: n.args.__setitem__(slice(0, 2), [n.args[1], n.args[0]])))
        elif (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
              and node.func.attr in ("softmax", "silu", "gelu", "relu") and node.args):
            muts.append((idx, line, f"remove activation .{node.func.attr}()",
                         lambda n: _replace_with(n, n.args[0])))
    return muts


def _replace_with(node, other):
    """In-place turn `node` into a copy of `other`."""
    node.__class__ = other.__class__
    node.__dict__ = copy.deepcopy(other.__dict__)


def apply_mutation(src: str, idx: int, fn) -> str:
    tree = ast.parse(src)
    order = []
    def walk(n):
        order.append(n)
        for c in ast.iter_child_nodes(n):
            walk(c)
    walk(tree)
    fn(order[idx])
    ast.fix_missing_locations(tree)
    return ast.unparse(tree) + "\n"


def run(cmd, cwd, timeout, pycache):
    env = dict(os.environ, PYTHONPYCACHEPREFIX=pycache, PYTHONDONTWRITEBYTECODE="1")
    try:
        p = subprocess.run(["bash", "-c", cmd], cwd=cwd, env=env, timeout=timeout,
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        return p.returncode, p.stdout
    except subprocess.TimeoutExpired:
        return 124, "timeout"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("module")
    ap.add_argument("tests")
    ap.add_argument("--cmd", default="python -m pytest {tests} -x -q -p no:cacheprovider")
    ap.add_argument("--cwd", default=".")
    ap.add_argument("--max-mutants", type=int, default=5)
    ap.add_argument("--min-score", type=float, default=0.8)
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--seed", type=int, default=0)
    a = ap.parse_args()

    module = os.path.abspath(a.module)
    cmd = a.cmd.format(tests=a.tests)
    original = open(module).read()
    backup = tempfile.NamedTemporaryFile("w", delete=False, suffix=".bak")
    backup.write(original); backup.close()

    def restore(*_):
        shutil.copyfile(backup.name, module)
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda s, f: (restore(), sys.exit(130)))

    try:
        rc, out = run(cmd, a.cwd, a.timeout, tempfile.mkdtemp())
        if rc != 0:
            print("[MUTATION_BASELINE_FAILED] tests do not pass unmutated\n" + out[-1500:])
            return 2

        muts = enumerate_mutations(ast.parse(original))
        if not muts:
            print("[MUTATION_NONE] no mutable sites found")
            return 2
        rng = random.Random(a.seed)
        # spread across the file: one per distinct line first, then fill randomly
        by_line = {}
        for m in muts:
            by_line.setdefault(m[1], []).append(m)
        picks = [rng.choice(v) for v in by_line.values()]
        rng.shuffle(picks)
        picks = picks[: a.max_mutants]

        killed, survivors, total = 0, [], 0
        for idx, line, desc, fn in picks:
            try:
                mutated = apply_mutation(original, idx, fn)
                compile(mutated, module, "exec")
            except Exception:
                continue  # invalid mutant, not counted
            total += 1
            open(module, "w").write(mutated)
            rc, _ = run(cmd, a.cwd, a.timeout, tempfile.mkdtemp())
            if rc == 0:
                survivors.append((line, desc))
            else:
                killed += 1
            restore()

        for line, desc in sorted(survivors):
            print(f"[MUTATION_SURVIVED] {os.path.basename(module)}:{line}  {desc}")
        score = killed / total if total else 0.0
        print(f"[MUTATION_SCORE] {killed}/{total} = {score:.2f}")
        return 0 if total and score >= a.min_score else 1
    finally:
        restore()
        os.unlink(backup.name)


if __name__ == "__main__":
    sys.exit(main())
