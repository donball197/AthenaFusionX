#!/usr/bin/env python3
"""
Verification script for Assessment algebra properties.
Tests ALL and ANY operation tables for commutativity, idempotency,
absorption, ordering, and duality.
"""

from enum import Enum
from itertools import product

class Assessment(Enum):
    SC = "SC"
    SP = "SP"
    RC = "RC"
    RP = "RP"
    NP = "NP"
    NN = "NN"

    def __str__(self):
        return self.value

# Dual mapping
DUAL = {
    Assessment.SC: Assessment.RC,
    Assessment.RC: Assessment.SC,
    Assessment.SP: Assessment.RP,
    Assessment.RP: Assessment.SP,
    Assessment.NP: Assessment.NP,
    Assessment.NN: Assessment.NN,
}

def dual(a: Assessment) -> Assessment:
    return DUAL[a]

# ALL operation table (from Rust implementation)
def all_op(a: Assessment, b: Assessment) -> Assessment:
    # RC is absorbing
    if a == Assessment.RC or b == Assessment.RC:
        return Assessment.RC
    # RP is absorbing (after RC check)
    if a == Assessment.RP or b == Assessment.RP:
        return Assessment.RP
    
    # Non-rejection states ordering: SC > SP > NP > NN
    # The result is the "minimum" in this ordering
    ordering = {
        Assessment.SC: 4,
        Assessment.SP: 3,
        Assessment.NP: 2,
        Assessment.NN: 1,
    }
    
    # Both must be non-rejection states here
    return min([a, b], key=lambda x: ordering[x])

# ANY derived from ALL via duality
def any_op(a: Assessment, b: Assessment) -> Assessment:
    return dual(all_op(dual(a), dual(b)))

# All states
STATES = list(Assessment)

def print_table(op_func, name):
    print(f"\n{name} table:")
    print("     " + "  ".join(f"{s.value:>2}" for s in STATES))
    for a in STATES:
        row = [f"{a.value:>2}"]
        for b in STATES:
            row.append(f"{op_func(a, b).value:>2}")
        print("  ".join(row))

def check_commutative(op_func, name):
    print(f"\n--- Checking commutativity for {name} ---")
    violations = []
    for a, b in product(STATES, STATES):
        if op_func(a, b) != op_func(b, a):
            violations.append((a, b, op_func(a, b), op_func(b, a)))
    if violations:
        print(f"  VIOLATIONS ({len(violations)}):")
        for a, b, ab, ba in violations:
            print(f"    {a.value} {name} {b.value} = {ab.value}, but {b.value} {name} {a.value} = {ba.value}")
        return False
    else:
        print("  PASS: Commutative")
        return True

def check_idempotent(op_func, name):
    print(f"\n--- Checking idempotency for {name} ---")
    violations = []
    for a in STATES:
        if op_func(a, a) != a:
            violations.append((a, op_func(a, a)))
    if violations:
        print(f"  VIOLATIONS ({len(violations)}):")
        for a, aa in violations:
            print(f"    {a.value} {name} {a.value} = {aa.value} (expected {a.value})")
        return False
    else:
        print("  PASS: Idempotent")
        return True

def check_absorbing(op_func, absorbing_state, name):
    print(f"\n--- Checking {absorbing_state.value} is absorbing for {name} ---")
    violations = []
    for a in STATES:
        if op_func(a, absorbing_state) != absorbing_state:
            violations.append((a, absorbing_state, op_func(a, absorbing_state)))
        if op_func(absorbing_state, a) != absorbing_state:
            violations.append((absorbing_state, a, op_func(absorbing_state, a)))
    if violations:
        print(f"  VIOLATIONS ({len(violations)}):")
        for a, b, result in violations:
            print(f"    {a.value} {name} {b.value} = {result.value} (expected {absorbing_state.value})")
        return False
    else:
        print(f"  PASS: {absorbing_state.value} is absorbing")
        return True

def check_ordering():
    print("\n--- Checking SC > SP > NP > NN ordering for non-rejection states ---")
    # For non-rejection states, ALL should return the "minimum" in the ordering
    # SC > SP > NP > NN means SC is "highest", NN is "lowest"
    # ALL(SC, SP) = SP, ALL(SP, NP) = NP, etc.
    ordering = [Assessment.SC, Assessment.SP, Assessment.NP, Assessment.NN]
    violations = []
    for i, a in enumerate(ordering):
        for j, b in enumerate(ordering):
            expected = ordering[max(i, j)]  # min in ordering = max index
            result = all_op(a, b)
            if result != expected:
                violations.append((a, b, result, expected))
    if violations:
        print(f"  VIOLATIONS ({len(violations)}):")
        for a, b, result, expected in violations:
            print(f"    ALL({a.value}, {b.value}) = {result.value} (expected {expected.value})")
        return False
    else:
        print("  PASS: Ordering holds")
        return True

def check_duality():
    print("\n--- Checking duality: ANY(a,b) = dual(ALL(dual(a), dual(b))) ---")
    violations = []
    for a, b in product(STATES, STATES):
        expected = dual(all_op(dual(a), dual(b)))
        actual = any_op(a, b)
        if actual != expected:
            violations.append((a, b, actual, expected))
    if violations:
        print(f"  VIOLATIONS ({len(violations)}):")
        for a, b, actual, expected in violations:
            print(f"    ANY({a.value}, {b.value}) = {actual.value} (expected {expected.value})")
        return False
    else:
        print("  PASS: Duality holds for all 36 pairs")
        return True

def check_dual_involution():
    print("\n--- Checking dual is an involution (dual(dual(x)) = x) ---")
    violations = []
    for a in STATES:
        if dual(dual(a)) != a:
            violations.append((a, dual(dual(a))))
    if violations:
        print(f"  VIOLATIONS ({len(violations)}):")
        for a, dda in violations:
            print(f"    dual(dual({a.value})) = {dda.value} (expected {a.value})")
        return False
    else:
        print("  PASS: Dual is an involution")
        return True

def main():
    print("=" * 60)
    print("ASSESSMENT ALGEBRA VERIFICATION")
    print("=" * 60)
    
    # Print tables
    print_table(all_op, "ALL")
    print_table(any_op, "ANY")
    
    # Run all checks
    all_pass = True
    
    all_pass &= check_commutative(all_op, "ALL")
    all_pass &= check_idempotent(all_op, "ALL")
    all_pass &= check_absorbing(all_op, Assessment.RC, "ALL")
    all_pass &= check_ordering()
    
    all_pass &= check_commutative(any_op, "ANY")
    all_pass &= check_idempotent(any_op, "ANY")
    all_pass &= check_absorbing(any_op, Assessment.SC, "ANY")
    
    all_pass &= check_duality()
    all_pass &= check_dual_involution()
    
    print("\n" + "=" * 60)
    if all_pass:
        print("ALL CHECKS PASSED")
    else:
        print("SOME CHECKS FAILED")
    print("=" * 60)
    
    return 0 if all_pass else 1

if __name__ == "__main__":
    exit(main())