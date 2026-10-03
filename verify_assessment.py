#!/usr/bin/env python3
"""
Verification script for Assessment algebra properties.
Checks commutativity, idempotence, absorption, ANY table, De Morgan duality.
"""

from enum import Enum

class Assessment(Enum):
    SC = "SC"
    SP = "SP"
    RC = "RC"
    RP = "RP"
    NP = "NP"
    NN = "NN"

# Dual mapping as specified
DUAL = {
    Assessment.SC: Assessment.RC,
    Assessment.RC: Assessment.SC,
    Assessment.SP: Assessment.RP,
    Assessment.RP: Assessment.SP,
    Assessment.NP: Assessment.NN,
    Assessment.NN: Assessment.NP,
}

def dual(a: Assessment) -> Assessment:
    return DUAL[a]

# ALL table from specification (rows = left, cols = right)
ALL_TABLE = {
    (Assessment.SC, Assessment.SC): Assessment.SC,
    (Assessment.SC, Assessment.SP): Assessment.SP,
    (Assessment.SC, Assessment.RC): Assessment.RC,
    (Assessment.SC, Assessment.RP): Assessment.RP,
    (Assessment.SC, Assessment.NP): Assessment.NP,
    (Assessment.SC, Assessment.NN): Assessment.NN,

    (Assessment.SP, Assessment.SC): Assessment.SP,
    (Assessment.SP, Assessment.SP): Assessment.SP,
    (Assessment.SP, Assessment.RC): Assessment.RC,
    (Assessment.SP, Assessment.RP): Assessment.RP,
    (Assessment.SP, Assessment.NP): Assessment.NP,
    (Assessment.SP, Assessment.NN): Assessment.NN,

    (Assessment.RC, Assessment.SC): Assessment.RC,
    (Assessment.RC, Assessment.SP): Assessment.RC,
    (Assessment.RC, Assessment.RC): Assessment.RC,
    (Assessment.RC, Assessment.RP): Assessment.RC,
    (Assessment.RC, Assessment.NP): Assessment.RC,
    (Assessment.RC, Assessment.NN): Assessment.RC,

    (Assessment.RP, Assessment.SC): Assessment.RP,
    (Assessment.RP, Assessment.SP): Assessment.RP,
    (Assessment.RP, Assessment.RC): Assessment.RC,
    (Assessment.RP, Assessment.RP): Assessment.RP,
    (Assessment.RP, Assessment.NP): Assessment.RP,
    (Assessment.RP, Assessment.NN): Assessment.RP,

    (Assessment.NP, Assessment.SC): Assessment.NP,
    (Assessment.NP, Assessment.SP): Assessment.NP,
    (Assessment.NP, Assessment.RC): Assessment.RC,
    (Assessment.NP, Assessment.RP): Assessment.RP,
    (Assessment.NP, Assessment.NP): Assessment.NP,
    (Assessment.NP, Assessment.NN): Assessment.NN,

    (Assessment.NN, Assessment.SC): Assessment.NN,
    (Assessment.NN, Assessment.SP): Assessment.NN,
    (Assessment.NN, Assessment.RC): Assessment.RC,
    (Assessment.NN, Assessment.RP): Assessment.RP,
    (Assessment.NN, Assessment.NP): Assessment.NN,
    (Assessment.NN, Assessment.NN): Assessment.NN,
}

def all_op(a: Assessment, b: Assessment) -> Assessment:
    return ALL_TABLE[(a, b)]

def any_op(a: Assessment, b: Assessment) -> Assessment:
    return dual(all_op(dual(a), dual(b)))

def print_table(op_name: str, op_func):
    states = [Assessment.SC, Assessment.SP, Assessment.RC, Assessment.RP, Assessment.NP, Assessment.NN]
    print(f"\n{op_name} table:")
    print("       " + "  ".join(f"{s.value:>2}" for s in states))
    for a in states:
        row = [op_func(a, b).value for b in states]
        print(f"  {a.value:>2}  " + "  ".join(f"{v:>2}" for v in row))

def check_commutative(op_func, op_name: str):
    states = list(Assessment)
    ok = True
    for a in states:
        for b in states:
            if op_func(a, b) != op_func(b, a):
                print(f"  FAIL: {op_name}({a.value}, {b.value}) = {op_func(a, b).value} != {op_func(b, a).value} = {op_name}({b.value}, {a.value})")
                ok = False
    if ok:
        print(f"  PASS: {op_name} is commutative")
    return ok

def check_idempotent(op_func, op_name: str):
    states = list(Assessment)
    ok = True
    for a in states:
        if op_func(a, a) != a:
            print(f"  FAIL: {op_name}({a.value}, {a.value}) = {op_func(a, a).value} != {a.value}")
            ok = False
    if ok:
        print(f"  PASS: {op_name} is idempotent")
    return ok

def check_absorbing(op_func, absorbing: Assessment, op_name: str):
    states = list(Assessment)
    ok = True
    for a in states:
        if op_func(a, absorbing) != absorbing:
            print(f"  FAIL: {op_name}({a.value}, {absorbing.value}) = {op_func(a, absorbing).value} != {absorbing.value}")
            ok = False
        if op_func(absorbing, a) != absorbing:
            print(f"  FAIL: {op_name}({absorbing.value}, {a.value}) = {op_func(absorbing, a).value} != {absorbing.value}")
            ok = False
    if ok:
        print(f"  PASS: {absorbing.value} is absorbing for {op_name}")
    return ok

def check_de_morgan():
    states = list(Assessment)
    ok = True
    for a in states:
        for b in states:
            lhs = dual(all_op(a, b))
            rhs = any_op(dual(a), dual(b))
            if lhs != rhs:
                print(f"  FAIL: dual(ALL({a.value}, {b.value})) = {lhs.value} != ANY(dual({a.value}), dual({b.value})) = {rhs.value}")
                ok = False
    if ok:
        print("  PASS: De Morgan duality holds for all 36 pairs")
    return ok

def check_order_reversal():
    """Check that dual reverses the order RC < RP < NN < NP < SP < SC"""
    # The order from weakest to strongest: RC < RP < NN < NP < SP < SC
    order = [Assessment.RC, Assessment.RP, Assessment.NN, Assessment.NP, Assessment.SP, Assessment.SC]
    ok = True
    for i in range(len(order)):
        for j in range(i+1, len(order)):
            a, b = order[i], order[j]
            # a < b means a is weaker than b
            # dual(a) should be > dual(b) (stronger)
            da, db = dual(a), dual(b)
            # Find positions in order
            pos_da = order.index(da)
            pos_db = order.index(db)
            if pos_da <= pos_db:
                print(f"  FAIL: Order reversal: {a.value} < {b.value} but dual({a.value})={da.value} not > dual({b.value})={db.value}")
                ok = False
    if ok:
        print("  PASS: Dual reverses the order RC < RP < NN < NP < SP < SC")
    return ok

def main():
    print("=" * 60)
    print("ASSESSMENT ALGEBRA VERIFICATION")
    print("=" * 60)

    # 1. Verify ALL is commutative and idempotent
    print("\n1. Checking ALL properties:")
    all_comm = check_commutative(all_op, "ALL")
    all_idem = check_idempotent(all_op, "ALL")

    # 2. Verify RC is absorbing for ALL
    print("\n2. Checking RC absorption for ALL:")
    rc_absorb = check_absorbing(all_op, Assessment.RC, "ALL")

    # 3. Compute full 6x6 ANY table
    print("\n3. ANY table (computed via duality):")
    print_table("ANY", any_op)

    # 4. Verify SC is absorbing for ANY
    print("\n4. Checking SC absorption for ANY:")
    sc_absorb = check_absorbing(any_op, Assessment.SC, "ANY")

    # 5. Check De Morgan duality for all 36 pairs
    print("\n5. Checking De Morgan duality:")
    dm_ok = check_de_morgan()

    # 6. Check order reversal
    print("\n6. Checking dual order reversal:")
    order_ok = check_order_reversal()

    # Summary
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    all_pass = all([all_comm, all_idem, rc_absorb, sc_absorb, dm_ok, order_ok])
    print(f"ALL commutative:     {'PASS' if all_comm else 'FAIL'}")
    print(f"ALL idempotent:      {'PASS' if all_idem else 'FAIL'}")
    print(f"RC absorbing (ALL):  {'PASS' if rc_absorb else 'FAIL'}")
    print(f"SC absorbing (ANY):  {'PASS' if sc_absorb else 'FAIL'}")
    print(f"De Morgan duality:   {'PASS' if dm_ok else 'FAIL'}")
    print(f"Order reversal:      {'PASS' if order_ok else 'FAIL'}")
    print(f"\nOVERALL: {'ALL CHECKS PASSED' if all_pass else 'SOME CHECKS FAILED'}")

    return 0 if all_pass else 1

if __name__ == "__main__":
    exit(main())
