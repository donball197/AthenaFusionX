#!/usr/bin/env python3
"""
Verify algebraic properties of the Assessment enum ALL and ANY operations.
Based on the nexus_evidence_requirements crate's algebra_tables.rs tests.
"""

from enum import Enum
from typing import Dict, List, Tuple

class Assessment(Enum):
    SC = "SC"
    SP = "SP"
    RC = "RC"
    RP = "RP"
    NP = "NP"
    NN = "NN"

    def __str__(self):
        return self.value

# ALL operation table (rows = left, cols = right)
ALL_TABLE: Dict[Assessment, Dict[Assessment, Assessment]] = {
    Assessment.SC: {
        Assessment.SC: Assessment.SC,
        Assessment.SP: Assessment.SP,
        Assessment.RC: Assessment.RC,
        Assessment.RP: Assessment.RP,
        Assessment.NP: Assessment.NP,
        Assessment.NN: Assessment.NN,
    },
    Assessment.SP: {
        Assessment.SC: Assessment.SP,
        Assessment.SP: Assessment.SP,
        Assessment.RC: Assessment.RC,
        Assessment.RP: Assessment.RP,
        Assessment.NP: Assessment.NP,
        Assessment.NN: Assessment.NN,
    },
    Assessment.RC: {
        Assessment.SC: Assessment.RC,
        Assessment.SP: Assessment.RC,
        Assessment.RC: Assessment.RC,
        Assessment.RP: Assessment.RC,
        Assessment.NP: Assessment.RC,
        Assessment.NN: Assessment.RC,
    },
    Assessment.RP: {
        Assessment.SC: Assessment.RP,
        Assessment.SP: Assessment.RP,
        Assessment.RC: Assessment.RC,
        Assessment.RP: Assessment.RP,
        Assessment.NP: Assessment.RP,
        Assessment.NN: Assessment.RP,
    },
    Assessment.NP: {
        Assessment.SC: Assessment.NP,
        Assessment.SP: Assessment.NP,
        Assessment.RC: Assessment.RC,
        Assessment.RP: Assessment.RP,
        Assessment.NP: Assessment.NP,
        Assessment.NN: Assessment.NN,
    },
    Assessment.NN: {
        Assessment.SC: Assessment.NN,
        Assessment.SP: Assessment.NN,
        Assessment.RC: Assessment.RC,
        Assessment.RP: Assessment.RP,
        Assessment.NP: Assessment.NN,
        Assessment.NN: Assessment.NN,
    },
}

# Dual mapping
DUAL: Dict[Assessment, Assessment] = {
    Assessment.SC: Assessment.RC,
    Assessment.RC: Assessment.SC,
    Assessment.SP: Assessment.RP,
    Assessment.RP: Assessment.SP,
    Assessment.NP: Assessment.NP,
    Assessment.NN: Assessment.NN,
}

def all_op(a: Assessment, b: Assessment) -> Assessment:
    return ALL_TABLE[a][b]

def dual(a: Assessment) -> Assessment:
    return DUAL[a]

def any_op(a: Assessment, b: Assessment) -> Assessment:
    """ANY(a,b) = dual( ALL( dual(a), dual(b) ) )"""
    return dual(all_op(dual(a), dual(b)))

def verify_commutative(op_func, name: str) -> bool:
    """Verify operation is commutative: op(a,b) == op(b,a) for all a,b"""
    print(f"\n=== Verifying {name} is commutative ===")
    all_states = list(Assessment)
    ok = True
    for a in all_states:
        for b in all_states:
            if op_func(a, b) != op_func(b, a):
                print(f"  FAIL: {name}({a}, {b}) = {op_func(a, b)} != {op_func(b, a)} = {name}({b}, {a})")
                ok = False
    if ok:
        print(f"  PASS: {name} is commutative")
    return ok

def verify_idempotent(op_func, name: str) -> bool:
    """Verify operation is idempotent: op(a,a) == a for all a"""
    print(f"\n=== Verifying {name} is idempotent ===")
    all_states = list(Assessment)
    ok = True
    for a in all_states:
        if op_func(a, a) != a:
            print(f"  FAIL: {name}({a}, {a}) = {op_func(a, a)} != {a}")
            ok = False
    if ok:
        print(f"  PASS: {name} is idempotent")
    return ok

def verify_absorbing(absorbing: Assessment, op_func, name: str) -> bool:
    """Verify absorbing element: op(absorbing, x) = absorbing and op(x, absorbing) = absorbing for all x"""
    print(f"\n=== Verifying {absorbing} is absorbing for {name} ===")
    all_states = list(Assessment)
    ok = True
    for x in all_states:
        left = op_func(absorbing, x)
        right = op_func(x, absorbing)
        if left != absorbing:
            print(f"  FAIL: {name}({absorbing}, {x}) = {left} != {absorbing}")
            ok = False
        if right != absorbing:
            print(f"  FAIL: {name}({x}, {absorbing}) = {right} != {absorbing}")
            ok = False
    if ok:
        print(f"  PASS: {absorbing} is absorbing for {name}")
    return ok

def verify_ordering() -> bool:
    """Verify SC > SP > NP > NN ordering for non-rejection states"""
    print(f"\n=== Verifying SC > SP > NP > NN ordering ===")
    # For non-rejection states (SC, SP, NP, NN), check that ALL respects the ordering
    # i.e., if a > b then ALL(a, b) = a and ALL(b, a) = a
    ordering = [Assessment.SC, Assessment.SP, Assessment.NP, Assessment.NN]
    ok = True
    for i, a in enumerate(ordering):
        for j, b in enumerate(ordering):
            if i < j:  # a > b
                result = all_op(a, b)
                if result != a:
                    print(f"  FAIL: ALL({a}, {b}) = {result} != {a} (expected a since {a} > {b})")
                    ok = False
                result = all_op(b, a)
                if result != a:
                    print(f"  FAIL: ALL({b}, {a}) = {result} != {a} (expected a since {a} > {b})")
                    ok = False
    if ok:
        print(f"  PASS: SC > SP > NP > NN ordering holds for ALL")
    return ok

def compute_any_table() -> Dict[Assessment, Dict[Assessment, Assessment]]:
    """Compute the full 6x6 ANY table using duality"""
    print(f"\n=== Computing ANY table (6x6) ===")
    all_states = list(Assessment)
    any_table = {}
    for a in all_states:
        any_table[a] = {}
        for b in all_states:
            any_table[a][b] = any_op(a, b)
    
    # Print table
    print("ANY table (rows=left, cols=right):")
    header = "      " + "  ".join(f"{s.value:>2}" for s in all_states)
    print(header)
    for a in all_states:
        row = f"{a.value:>4}  " + "  ".join(f"{any_table[a][b].value:>2}" for b in all_states)
        print(row)
    return any_table

def verify_duality() -> bool:
    """Check duality for all 36 pairs: dual(ALL(a,b)) == ANY(dual(a), dual(b))"""
    print(f"\n=== Verifying duality for all 36 pairs ===")
    all_states = list(Assessment)
    ok = True
    for a in all_states:
        for b in all_states:
            left = dual(all_op(a, b))
            right = any_op(dual(a), dual(b))
            if left != right:
                print(f"  FAIL: dual(ALL({a}, {b})) = {left} != {right} = ANY(dual({a}), dual({b}))")
                ok = False
    if ok:
        print(f"  PASS: Duality holds for all 36 pairs")
    return ok

def print_all_table():
    """Print the ALL table for reference"""
    print("ALL table (rows=left, cols=right):")
    all_states = list(Assessment)
    header = "      " + "  ".join(f"{s.value:>2}" for s in all_states)
    print(header)
    for a in all_states:
        row = f"{a.value:>4}  " + "  ".join(f"{ALL_TABLE[a][b].value:>2}" for b in all_states)
        print(row)

def main():
    print("=" * 60)
    print("ALGEBRAIC VERIFICATION OF ASSESSMENT OPERATIONS")
    print("=" * 60)
    
    print_all_table()
    
    # Task 1: Verify ALL is commutative and idempotent
    all_comm = verify_commutative(all_op, "ALL")
    all_idem = verify_idempotent(all_op, "ALL")
    
    # Task 2: Verify RC is absorbing for ALL
    rc_absorbs = verify_absorbing(Assessment.RC, all_op, "ALL")
    
    # Task 3: Verify SC > SP > NP > NN ordering
    ordering_ok = verify_ordering()
    
    # Task 4: Compute full 6x6 ANY table
    any_table = compute_any_table()
    
    # Task 5: Check ANY is commutative and idempotent
    any_comm = verify_commutative(any_op, "ANY")
    any_idem = verify_idempotent(any_op, "ANY")
    
    # Task 6: Verify SC is absorbing for ANY
    sc_absorbs = verify_absorbing(Assessment.SC, any_op, "ANY")
    
    # Task 7: Check duality for all 36 pairs
    duality_ok = verify_duality()
    
    # Task 8: Report any contradictions or edge cases
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    results = [
        ("ALL commutative", all_comm),
        ("ALL idempotent", all_idem),
        ("RC absorbs ALL", rc_absorbs),
        ("SC > SP > NP > NN ordering", ordering_ok),
        ("ANY commutative", any_comm),
        ("ANY idempotent", any_idem),
        ("SC absorbs ANY", sc_absorbs),
        ("Duality (36 pairs)", duality_ok),
    ]
    
    all_pass = True
    for name, result in results:
        status = "PASS" if result else "FAIL"
        print(f"  {status}: {name}")
        if not result:
            all_pass = False
    
    print("=" * 60)
    if all_pass:
        print("ALL VERIFICATIONS PASSED - No contradictions found")
    else:
        print("SOME VERIFICATIONS FAILED - Contradictions detected")
    print("=" * 60)
    
    return all_pass

if __name__ == "__main__":
    success = main()
    exit(0 if success else 1)