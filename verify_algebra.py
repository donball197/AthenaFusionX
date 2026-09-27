import json

states = ["SC", "SP", "RC", "RP", "NP", "NN"]
dual_map = {
    "SC": "RC", "RC": "SC",
    "SP": "RP", "RP": "SP",
    "NP": "NP", "NN": "NN"
}

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

def dual(a):
    return dual_map[a]

def ANY(a, b):
    return dual(ALL(dual(a), dual(b)))

results = {}

# 1. Verify ALL is commutative and idempotent
all_commutative = True
all_idempotent = True
for a in states:
    if ALL(a, a) != a:
        all_idempotent = False
    for b in states:
        if ALL(a, b) != ALL(b, a):
            all_commutative = False

results["all_commutative"] = all_commutative
results["all_idempotent"] = all_idempotent

# 2. Verify RC is absorbing for ALL
rc_absorbing = True
for a in states:
    if ALL(a, "RC") != "RC" or ALL("RC", a) != "RC":
        rc_absorbing = False
results["rc_absorbing"] = rc_absorbing

# 3. Verify SC > SP > NP > NN ordering for non-rejection states
# Non-rejection states: SC, SP, NP, NN
# Ordering means ALL(a, b) = min(a, b) under this ordering.
ordering = {"SC": 4, "SP": 3, "NP": 2, "NN": 1}
ordering_valid = True
for a in ordering:
    for b in ordering:
        expected = a if ordering[a] <= ordering[b] else b
        if ALL(a, b) != expected:
            ordering_valid = False
results["ordering_valid"] = ordering_valid

# 4. Compute the full 6x6 ANY table using duality
any_table = {}
for a in states:
    any_table[a] = {}
    for b in states:
        any_table[a][b] = ANY(a, b)
results["any_table"] = any_table

# 5. Check ANY is commutative and idempotent
any_commutative = True
any_idempotent = True
for a in states:
    if ANY(a, a) != a:
        any_idempotent = False
    for b in states:
        if ANY(a, b) != ANY(b, a):
            any_commutative = False
results["any_commutative"] = any_commutative
results["any_idempotent"] = any_idempotent

# 6. Verify SC is absorbing for ANY
sc_absorbing_any = True
for a in states:
    if ANY(a, "SC") != "SC" or ANY("SC", a) != "SC":
        sc_absorbing_any = False
results["sc_absorbing_any"] = sc_absorbing_any

# 7. Check duality for all 36 pairs
duality_valid = True
for a in states:
    for b in states:
        if dual(ALL(a, b)) != ANY(dual(a), dual(b)):
            duality_valid = False
results["duality_valid"] = duality_valid

print(json.dumps(results, indent=2))
