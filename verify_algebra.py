# verify_algebra.py

states = ["SC", "SP", "RC", "RP", "NP", "NN"]

duals = {
    "SC": "RC",
    "RC": "SC",
    "SP": "RP",
    "RP": "SP",
    "NP": "NP",
    "NN": "NN"
}

# ALL table (rows = left, cols = right):
#         SC  SP  RC  RP  NP  NN
# SC      SC  SP  RC  RP  NP  NN
# SP      SP  SP  RC  RP  NP  NN
# RC      RC  RC  RC  RC  RC  RC
# RP      RP  RP  RC  RP  RP  RP
# NP      NP  NP  RC  RP  NP  NN
# NN      NN  NN  RC  RP  NN  NN

all_table = {
    "SC": {"SC": "SC", "SP": "SP", "RC": "RC", "RP": "RP", "NP": "NP", "NN": "NN"},
    "SP": {"SC": "SP", "SP": "SP", "RC": "RC", "RP": "RP", "NP": "NP", "NN": "NN"},
    "RC": {"SC": "RC", "SP": "RC", "RC": "RC", "RP": "RC", "NP": "RC", "NN": "RC"},
    "RP": {"SC": "RP", "SP": "RP", "RC": "RC", "RP": "RP", "NP": "RP", "NN": "RP"},
    "NP": {"SC": "NP", "SP": "NP", "RC": "RC", "RP": "RP", "NP": "NP", "NN": "NN"},
    "NN": {"SC": "NN", "SP": "NN", "RC": "RC", "RP": "RP", "NP": "NN", "NN": "NN"},
}

def ALL(a, b):
    return all_table[a][b]

def dual(a):
    return duals[a]

def ANY(a, b):
    return dual(ALL(dual(a), dual(b)))

# 1. Verify ALL is commutative and idempotent
all_commutative = True
all_idempotent = True
for a in states:
    for b in states:
        if ALL(a, b) != ALL(b, a):
            print(f"ALL Commutativity failure: ALL({a}, {b}) = {ALL(a, b)} != ALL({b}, {a}) = {ALL(b, a)}")
            all_commutative = False
    if ALL(a, a) != a:
        print(f"ALL Idempotency failure: ALL({a}, {a}) = {ALL(a, a)} != {a}")
        all_idempotent = False

# 2. Verify RC is absorbing for ALL
rc_absorbing = True
for a in states:
    if ALL(a, "RC") != "RC" or ALL("RC", a) != "RC":
        print(f"RC absorbing failure for ALL: ALL({a}, RC) = {ALL(a, 'RC')}, ALL(RC, {a}) = {ALL('RC', a)}")
        rc_absorbing = False

# 3. Verify SC > SP > NP > NN ordering for non-rejection states
# Let's define the ordering and check if ALL(a, b) is the minimum (or maximum) of a and b.
# If SC > SP > NP > NN, then ALL(a, b) should be the minimum of a and b.
# Let's check if ALL(a, b) behaves as a meet (infimum) for this ordering.
# Let's define the order: SC=3, SP=2, NP=1, NN=0.
order = {"SC": 3, "SP": 2, "NP": 1, "NN": 0}
ordering_ok = True
non_rejection = ["SC", "SP", "NP", "NN"]
for a in non_rejection:
    for b in non_rejection:
        expected_val = a if order[a] < order[b] else b
        # Wait, let's check if ALL(a, b) matches the expected minimum
        actual_val = ALL(a, b)
        if actual_val != expected_val:
            print(f"Ordering mismatch: ALL({a}, {b}) = {actual_val}, expected min = {expected_val}")
            ordering_ok = False

# 4. Compute the full 6x6 ANY table using duality
any_table = {}
for a in states:
    any_table[a] = {}
    for b in states:
        any_table[a][b] = ANY(a, b)

# Print ANY table
print("ANY table:")
print("        " + "  ".join(states))
for a in states:
    row = f"{a:<6}"
    for b in states:
        row += f"  {any_table[a][b]:<2}"
    print(row)

# 5. Check ANY is commutative and idempotent
any_commutative = True
any_idempotent = True
for a in states:
    for b in states:
        if ANY(a, b) != ANY(b, a):
            print(f"ANY Commutativity failure: ANY({a}, {b}) = {ANY(a, b)} != ANY({b}, {a}) = {ANY(b, a)}")
            any_commutative = False
    if ANY(a, a) != a:
        print(f"ANY Idempotency failure: ANY({a}, {a}) = {ANY(a, a)} != {a}")
        any_idempotent = False

# 6. Verify SC is absorbing for ANY
sc_absorbing = True
for a in states:
    if ANY(a, "SC") != "SC" or ANY("SC", a) != "SC":
        print(f"SC absorbing failure for ANY: ANY({a}, SC) = {ANY(a, 'SC')}, ANY(SC, {a}) = {ANY('SC', a)}")
        sc_absorbing = False

# 7. Check duality for all 36 pairs
duality_ok = True
for a in states:
    for b in states:
        # dual(ALL(a, b)) == ANY(dual(a), dual(b))
        lhs = dual(ALL(a, b))
        rhs = ANY(dual(a), dual(b))
        if lhs != rhs:
            print(f"Duality failure: dual(ALL({a}, {b})) = {lhs} != ANY(dual({a}), dual({b})) = {rhs}")
            duality_ok = False

print(f"ALL Commutative: {all_commutative}")
print(f"ALL Idempotent: {all_idempotent}")
print(f"RC Absorbing for ALL: {rc_absorbing}")
print(f"Ordering SC > SP > NP > NN OK: {ordering_ok}")
print(f"ANY Commutative: {any_commutative}")
print(f"ANY Idempotent: {any_idempotent}")
print(f"SC Absorbing for ANY: {sc_absorbing}")
print(f"Duality OK: {duality_ok}")
