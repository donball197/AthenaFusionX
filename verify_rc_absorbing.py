#!/usr/bin/env python3
"""
Verify RC is absorbing for ALL across all 36 state pairs.
"""

# Define the 6 states
STATES = ['SC', 'SP', 'RC', 'RP', 'NP', 'NN']

# ALL fusion table (rows = left, cols = right)
ALL_TABLE = {
    'SC': {'SC': 'SC', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'SP': {'SC': 'SP', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'RC': {'SC': 'RC', 'SP': 'RC', 'RC': 'RC', 'RP': 'RC', 'NP': 'RC', 'NN': 'RC'},
    'RP': {'SC': 'RP', 'SP': 'RP', 'RC': 'RC', 'RP': 'RP', 'NP': 'RP', 'NN': 'RP'},
    'NP': {'SC': 'NP', 'SP': 'NP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'NN': {'SC': 'NN', 'SP': 'NN', 'RC': 'RC', 'RP': 'RP', 'NP': 'NN', 'NN': 'NN'},
}

def all_fusion(a, b):
    """Compute ALL(a, b) using the table."""
    return ALL_TABLE[a][b]

def verify_rc_absorbing():
    """Verify RC is absorbing for ALL: ALL(x, RC) == RC and ALL(RC, x) == RC for all x."""
    print("Verifying RC is absorbing for ALL...")
    print("=" * 50)
    
    all_passed = True
    
    for x in STATES:
        # Check ALL(x, RC) == RC
        result_left = all_fusion(x, 'RC')
        left_ok = result_left == 'RC'
        
        # Check ALL(RC, x) == RC
        result_right = all_fusion('RC', x)
        right_ok = result_right == 'RC'
        
        status = "PASS" if (left_ok and right_ok) else "FAIL"
        if not (left_ok and right_ok):
            all_passed = False
        
        print(f"  ALL({x}, RC) = {result_left} {'✓' if left_ok else '✗'}")
        print(f"  ALL(RC, {x}) = {result_right} {'✓' if right_ok else '✗'}")
        print(f"  Status: {status}")
        print()
    
    print("=" * 50)
    if all_passed:
        print("✓ RC is absorbing for ALL - ALL checks PASSED")
    else:
        print("✗ RC is NOT absorbing for ALL - some checks FAILED")
    
    return all_passed

def verify_commutative():
    """Verify ALL is commutative: ALL(a, b) == ALL(b, a) for all pairs."""
    print("\nVerifying ALL is commutative...")
    print("=" * 50)
    
    all_passed = True
    for a in STATES:
        for b in STATES:
            if all_fusion(a, b) != all_fusion(b, a):
                print(f"  FAIL: ALL({a}, {b}) = {all_fusion(a, b)} != ALL({b}, {a}) = {all_fusion(b, a)}")
                all_passed = False
    
    if all_passed:
        print("  ✓ ALL is commutative - all 36 pairs verified")
    else:
        print("  ✗ ALL is NOT commutative")
    
    return all_passed

def verify_idempotent():
    """Verify ALL is idempotent: ALL(a, a) == a for all states."""
    print("\nVerifying ALL is idempotent...")
    print("=" * 50)
    
    all_passed = True
    for a in STATES:
        result = all_fusion(a, a)
        ok = result == a
        if not ok:
            print(f"  FAIL: ALL({a}, {a}) = {result} != {a}")
            all_passed = False
        else:
            print(f"  ✓ ALL({a}, {a}) = {a}")
    
    if all_passed:
        print("  ✓ ALL is idempotent - all 6 states verified")
    else:
        print("  ✗ ALL is NOT idempotent")
    
    return all_passed

if __name__ == "__main__":
    print("RC Absorbing Verification for ALL Fusion Table")
    print("=" * 50)
    print()
    
    # Run all verifications
    rc_absorbing = verify_rc_absorbing()
    commutative = verify_commutative()
    idempotent = verify_idempotent()
    
    print("\n" + "=" * 50)
    print("SUMMARY")
    print("=" * 50)
    print(f"RC Absorbing:     {'PASS' if rc_absorbing else 'FAIL'}")
    print(f"Commutative:      {'PASS' if commutative else 'FAIL'}")
    print(f"Idempotent:       {'PASS' if idempotent else 'FAIL'}")
    
    if rc_absorbing and commutative and idempotent:
        print("\n✓ ALL VERIFICATIONS PASSED")
        exit(0)
    else:
        print("\n✗ SOME VERIFICATIONS FAILED")
        exit(1)