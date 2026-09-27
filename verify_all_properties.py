import sys

states = ["SC", "SP", "RC", "RP", "NP", "NN"]

dual_map = {
    "SC": "RC", "RC": "SC",
    "SP": "RP", "RP": "SP",
    "NP": "NP", "NN": "NN"
}

def dual(s):
    return dual_map[s]

# ALL table
# Rows = left, Cols = right
all_table = {
    "SC": {"SC": "SC", "SP": "SP", "RC": "RC", "RP": "RP", "NP": "NP", "NN": "NN"},
    "SP": {"SC": "SP", "SP": "SP", "RC": "RC", "RP": "RP", "NP": "NP", "NN": "NN"},
    "RC": {"SC": "RC", "SP": "RC", "RC": "RC", "RP": "RC", "NP": "RC", "NN": "RC"},
    "RP": {"SC": "RP", "SP": "RP", "RC": "RC", "RP": "RP", "NP": "RP", "NN": "RP"},
    "NP": {"SC": "NP", "SP": "NP", "RC": "RC", "RP": "RP", "NP": "NP", "NN": "NN"},
    "NN": {"SC": "NN", "SP": "NN", "RC": "RC", "RP": "RP", "NP": "NN", "NN": "NN"}
}

def ALL(a, b):
    return all_table[a][b]

def ANY(a, b):
    return dual(ALL(dual(a), dual(b)))

print("--- Task 1: Verify ALL is commutative and idempotent ---")
all_commutative = True
all_idempotent = True
for a in states:
    for b in states:
        if ALL(a, b) != ALL(b, a):
            print(f"ALL is NOT commutative: ALL({a}, {b}) = {ALL(a, b)} != ALL({b}, {a}) = {ALL(b, a)}")
            all_commutative = False
        if a == b and ALL(a, a) != a:
            print(f"ALL is NOT idempotent: ALL({a}, {a}) = {ALL(a, a)} != {a}")
            all_idempotent = False

print(f"ALL Commutative: {all_commutative}")
print(f"ALL Idempotent: {all_idempotent}")

print("\n--- Task 2: Verify RC is absorbing for ALL ---")
rc_absorbing = True
for a in states:
    if ALL(a, "RC") != "RC" or ALL("RC", a) != "RC":
        print(f"RC is NOT absorbing: ALL({a}, RC) = {ALL(a, 'RC')}, ALL(RC, {a}) = {ALL('RC', a)}")
        rc_absorbing = False
print(f"RC is absorbing for ALL: {rc_absorbing}")

print("\n--- Task 3: Verify SC > SP > NP > NN ordering for non-rejection states ---")
# SC=4, SP=3, NP=2, NN=1
order_val = {"SC": 4, "SP": 3, "NP": 2, "NN": 1}
non_rejection = ["SC", "SP", "NP", "NN"]
ordering_valid = True
for a in non_rejection:
    for b in non_rejection:
        expected = a if order_val[a] < order_val[b] else b
        actual = ALL(a, b)
        if actual != expected:
            print(f"Ordering mismatch: ALL({a}, {b}) = {actual}, expected min({a}, {b}) = {expected}")
            ordering_valid = False
print(f"SC > SP > NP > NN ordering valid (ALL acts as min): {ordering_valid}")

print("\n--- Task 4: Compute the full 6x6 ANY table using duality ---")
any_table = {}
print("    " + "  ".join(states))
for a in states:
    row_strs = []
    any_table[a] = {}
    for b in states:
        res = ANY(a, b)
        any_table[a][b] = res
        row_strs.append(res)
    print(f"{a:<4}" + "  ".join(row_strs))

print("\n--- Task 5: Check ANY is commutative and idempotent ---")
any_commutative = True
any_idempotent = True
for a in states:
    for b in states:
        if ANY(a, b) != ANY(b, a):
            print(f"ANY is NOT commutative: ANY({a}, {b}) = {ANY(a, b)} != ANY({b}, {a}) = {ANY(b, a)}")
            any_commutative = False
        if a == b and ANY(a, a) != a:
            print(f"ANY is NOT idempotent: ANY({a}, {a}) = {ANY(a, a)} != {a}")
            any_idempotent = False
print(f"ANY Commutative: {any_commutative}")
print(f"ANY Idempotent: {any_idempotent}")

print("\n--- Task 6: Verify SC is absorbing for ANY ---")
sc_absorbing = True
for a in states:
    if ANY(a, "SC") != "SC" or ANY("SC", a) != "SC":
        print(f"SC is NOT absorbing for ANY: ANY({a}, SC) = {ANY(a, 'SC')}, ANY(SC, {a}) = {ANY('SC', a)}")
        sc_absorbing = False
print(f"SC is absorbing for ANY: {sc_absorbing}")

print("\n--- Task 7: Check duality for all 36 pairs ---")
duality_valid = True
for a in states:
    for b in states:
        # dual(ALL(a, b)) == ANY(dual(a), dual(b))
        if dual(ALL(a, b)) != ANY(dual(a), dual(b)):
            print(f"Duality failed for ALL({a}, {b})")
            duality_valid = False
        # dual(ANY(a, b)) == ALL(dual(a), dual(b))
        if dual(ANY(a, b)) != ALL(dual(a), dual(b)):
            print(f"Duality failed for ANY({a}, {b})")
            duality_valid = False
print(f"Duality holds for all 36 pairs: {duality_valid}")

