import sys

states = ["SC", "SP", "RC", "RP", "NP", "NN"]
dual = {
    "SC": "RC", "RC": "SC",
    "SP": "RP", "RP": "SP",
    "NP": "NP",
    "NN": "NN"
}

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

def ANY(a, b):
    return dual[ALL(dual[a], dual[b])]

print("--- Task 1: Verify ALL is commutative and idempotent ---")
commutative_all = True
idempotent_all = True
for a in states:
    for b in states:
        if ALL(a, b) != ALL(b, a):
            print(f"ALL is NOT commutative: ALL({a}, {b}) = {ALL(a, b)} != ALL({b}, {a}) = {ALL(b, a)}")
            commutative_all = False
    if ALL(a, a) != a:
        print(f"ALL is NOT idempotent: ALL({a}, {a}) = {ALL(a, a)} != {a}")
        idempotent_all = False

if commutative_all:
    print("ALL is commutative.")
if idempotent_all:
    print("ALL is idempotent.")

print("\n--- Task 2: Verify RC is absorbing for ALL ---")
absorbing_rc = True
for a in states:
    if ALL(a, "RC") != "RC" or ALL("RC", a) != "RC":
        print(f"RC is NOT absorbing: ALL({a}, RC) = {ALL(a, 'RC')}, ALL(RC, {a}) = {ALL('RC', a)}")
        absorbing_rc = False
if absorbing_rc:
    print("RC is absorbing for ALL.")

print("\n--- Task 3: Verify SC > SP > NP > NN ordering for non-rejection states ---")
# Non-rejection states: SC, SP, NP, NN
# Ordering: SC > SP > NP > NN
# Let's check if ALL(a, b) = min(a, b) under this ordering.
# If SC > SP > NP > NN, then:
# ALL(SC, SP) = SP (min)
# ALL(SC, NP) = NP (min)
# ALL(SC, NN) = NN (min)
# ALL(SP, NP) = NP (min)
# ALL(SP, NN) = NN (min)
# ALL(NP, NN) = NN (min)
# Let's verify this.
ordering = {"SC": 4, "SP": 3, "NP": 2, "NN": 1}
non_rejection = ["SC", "SP", "NP", "NN"]
ordering_valid = True
for a in non_rejection:
    for b in non_rejection:
        expected = a if ordering[a] < ordering[b] else b
        if ALL(a, b) != expected:
            print(f"Ordering mismatch: ALL({a}, {b}) = {ALL(a, b)}, expected {expected}")
            ordering_valid = False
if ordering_valid:
    print("SC > SP > NP > NN ordering is verified for non-rejection states (ALL acts as min).")

print("\n--- Task 4: Compute the full 6x6 ANY table using duality ---")
any_table = {}
print("    " + "  ".join(states))
for a in states:
    row = []
    any_table[a] = {}
    for b in states:
        val = ANY(a, b)
        any_table[a][b] = val
        row.append(val)
    print(f"{a:<4}" + "  ".join(row))

print("\n--- Task 5: Check ANY is commutative and idempotent ---")
commutative_any = True
idempotent_any = True
for a in states:
    for b in states:
        if ANY(a, b) != ANY(b, a):
            print(f"ANY is NOT commutative: ANY({a}, {b}) = {ANY(a, b)} != ANY({b}, {a}) = {ANY(b, a)}")
            commutative_any = False
    if ANY(a, a) != a:
        print(f"ANY is NOT idempotent: ANY({a}, {a}) = {ANY(a, a)} != {a}")
        idempotent_any = False
if commutative_any:
    print("ANY is commutative.")
if idempotent_any:
    print("ANY is idempotent.")

print("\n--- Task 6: Verify SC is absorbing for ANY ---")
absorbing_sc = True
for a in states:
    if ANY(a, "SC") != "SC" or ANY("SC", a) != "SC":
        print(f"SC is NOT absorbing for ANY: ANY({a}, SC) = {ANY(a, 'SC')}, ANY(SC, {a}) = {ANY('SC', a)}")
        absorbing_sc = False
if absorbing_sc:
    print("SC is absorbing for ANY.")

print("\n--- Task 7: Check duality for all 36 pairs ---")
# Duality: ANY(a, b) = dual(ALL(dual(a), dual(b))) and ALL(a, b) = dual(ANY(dual(a), dual(b)))
duality_valid = True
for a in states:
    for b in states:
        if ANY(a, b) != dual[ALL(dual[a], dual[b])]:
            duality_valid = False
        if ALL(a, b) != dual[ANY(dual[a], dual[b])]:
            duality_valid = False
if duality_valid:
    print("Duality holds for all 36 pairs.")

print("\n--- Task 8: Report any contradictions or edge cases ---")
# No contradictions found if all checks passed.
if commutative_all and idempotent_all and absorbing_rc and ordering_valid and commutative_any and idempotent_any and absorbing_sc and duality_valid:
    print("All checks passed successfully! No contradictions or edge cases found.")
else:
    print("Contradictions or edge cases found!")
    sys.exit(1)
