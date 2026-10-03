#!/usr/bin/env python3
"""
Verify the strict ordering SC > SP > NP > NN for non-rejection states
under the defined ALL table.
"""

# ALL table as defined in the directive
# Rows = left operand, Cols = right operand
ALL_TABLE = {
    'SC': {'SC': 'SC', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'SP': {'SC': 'SP', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'RC': {'SC': 'RC', 'SP': 'RC', 'RC': 'RC', 'RP': 'RC', 'NP': 'RC', 'NN': 'RC'},
    'RP': {'SC': 'RP', 'SP': 'RP', 'RC': 'RC', 'RP': 'RP', 'NP': 'RP', 'NN': 'RP'},
    'NP': {'SC': 'NP', 'SP': 'NP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'NN': {'SC': 'NN', 'SP': 'NN', 'RC': 'RC', 'RP': 'RP', 'NP': 'NN', 'NN': 'NN'},
}

# Non-rejection states in order SC > SP > NP > NN
NON_REJECTION_STATES = ['SC', 'SP', 'NP', 'NN']

def all_op(a, b):
    """Compute ALL(a, b)"""
    return ALL_TABLE[a][b]

def verify_ordering():
    """Verify SC > SP > NP > NN ordering for non-rejection states."""
    print("=" * 60)
    print("Verifying SC > SP > NP > NN ordering for non-rejection states")
    print("=" * 60)
    
    # The ordering SC > SP > NP > NN means:
    # For any two states a, b where a comes before b in this list (a > b),
    # ALL(a, b) should equal b (the "lesser" state absorbs)
    # Since ALL is commutative, ALL(b, a) should also equal b
    
    all_passed = True
    
    for i, a in enumerate(NON_REJECTION_STATES):
        for j, b in enumerate(NON_REJECTION_STATES):
            if i == j:
                # Idempotent: ALL(a, a) = a
                result = all_op(a, b)
                expected = a
                status = "✓" if result == expected else "✗"
                if result != expected:
                    all_passed = False
                print(f"  ALL({a}, {b}) = {result} (expected {expected}) {status}")
            elif i < j:
                # a > b in ordering (a comes before b)
                # ALL(a, b) should = b (the lesser state)
                result = all_op(a, b)
                expected = b
                status = "✓" if result == expected else "✗"
                if result != expected:
                    all_passed = False
                print(f"  ALL({a}, {b}) = {result} (expected {expected} since {a} > {b}) {status}")
                
                # Commutative check
                result_rev = all_op(b, a)
                status_rev = "✓" if result_rev == expected else "✗"
                if result_rev != expected:
                    all_passed = False
                print(f"  ALL({b}, {a}) = {result_rev} (expected {expected}) {status_rev}")
    
    print()
    if all_passed:
        print("✓ ALL ordering checks PASSED: SC > SP > NP > NN verified")
    else:
        print("✗ ALL ordering checks FAILED")
    
    return all_passed

def verify_commutative():
    """Verify ALL is commutative for all states."""
    print("\n" + "=" * 60)
    print("Verifying ALL is commutative for all 6 states")
    print("=" * 60)
    
    states = ['SC', 'SP', 'RC', 'RP', 'NP', 'NN']
    all_passed = True
    
    for a in states:
        for b in states:
            if all_op(a, b) != all_op(b, a):
                print(f"  ✗ ALL({a}, {b}) = {all_op(a, b)} != ALL({b}, {a}) = {all_op(b, a)}")
                all_passed = False
    
    if all_passed:
        print("  ✓ ALL is commutative for all 36 pairs")
    return all_passed

def verify_idempotent():
    """Verify ALL is idempotent for all states."""
    print("\n" + "=" * 60)
    print("Verifying ALL is idempotent for all 6 states")
    print("=" * 60)
    
    states = ['SC', 'SP', 'RC', 'RP', 'NP', 'NN']
    all_passed = True
    
    for a in states:
        if all_op(a, a) != a:
            print(f"  ✗ ALL({a}, {a}) = {all_op(a, a)} != {a}")
            all_passed = False
    
    if all_passed:
        print("  ✓ ALL is idempotent for all 6 states")
    return all_passed

def verify_rc_absorbing():
    """Verify RC is absorbing for ALL."""
    print("\n" + "=" * 60)
    print("Verifying RC is absorbing for ALL")
    print("=" * 60)
    
    states = ['SC', 'SP', 'RC', 'RP', 'NP', 'NN']
    all_passed = True
    
    for a in states:
        if all_op(a, 'RC') != 'RC':
            print(f"  ✗ ALL({a}, RC) = {all_op(a, 'RC')} != RC")
            all_passed = False
        if all_op('RC', a) != 'RC':
            print(f"  ✗ ALL(RC, {a}) = {all_op('RC', a)} != RC")
            all_passed = False
    
    if all_passed:
        print("  ✓ RC is absorbing for ALL (ALL(x, RC) = RC for all x)")
    return all_passed

def main():
    print("ALL Table Verification")
    print("=" * 60)
    
    results = []
    results.append(("Commutative", verify_commutative()))
    results.append(("Idempotent", verify_idempotent()))
    results.append(("RC Absorbing", verify_rc_absorbing()))
    results.append(("SC > SP > NP > NN Ordering", verify_ordering()))
    
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    for name, passed in results:
        status = "PASS" if passed else "FAIL"
        print(f"  {name}: {status}")
    
    all_passed = all(passed for _, passed in results)
    print(f"\nOverall: {'ALL CHECKS PASSED' if all_passed else 'SOME CHECKS FAILED'}")
    
    return all_passed

if __name__ == "__main__":
    success = main()
    exit(0 if success else 1)