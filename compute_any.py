states = ['SC', 'SP', 'RC', 'RP', 'NP', 'NN']

dual_map = {
    'SC': 'RC', 'RC': 'SC',
    'SP': 'RP', 'RP': 'SP',
    'NP': 'NP', 'NN': 'NN'
}

all_table = {
    'SC': {'SC': 'SC', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'SP': {'SC': 'SP', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'RC': {'SC': 'RC', 'SP': 'RC', 'RC': 'RC', 'RP': 'RC', 'NP': 'RC', 'NN': 'RC'},
    'RP': {'SC': 'RP', 'SP': 'RP', 'RC': 'RC', 'RP': 'RP', 'NP': 'RP', 'NN': 'RP'},
    'NP': {'SC': 'NP', 'SP': 'NP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'NN': {'SC': 'NN', 'SP': 'NN', 'RC': 'RC', 'RP': 'RP', 'NP': 'NN', 'NN': 'NN'},
}

def dual(s):
    return dual_map[s]

def ALL(a, b):
    return all_table[a][b]

def ANY(a, b):
    return dual(ALL(dual(a), dual(b)))

# 1. Verify ALL is commutative and idempotent
all_commutative = all(ALL(a, b) == ALL(b, a) for a in states for b in states)
all_idempotent = all(ALL(a, a) == a for a in states)
print(f"ALL commutative: {all_commutative}")
print(f"ALL idempotent: {all_idempotent}")

# 2. Verify RC is absorbing for ALL
rc_absorbing = all(ALL(a, 'RC') == 'RC' and ALL('RC', a) == 'RC' for a in states)
print(f"RC is absorbing for ALL: {rc_absorbing}")

# 3. Verify SC > SP > NP > NN ordering for non-rejection states
# We can define a helper to check if ALL(a, b) behaves as a minimum under some ordering.
# For non-rejection states {SC, SP, NP, NN}, ALL(a, b) should be the minimum of a and b.
# Let's check if ALL(a, b) == b when a > b (i.e. a is higher in the order SC > SP > NP > NN).
order = {'SC': 4, 'SP': 3, 'NP': 2, 'NN': 1}
non_rejection = ['SC', 'SP', 'NP', 'NN']
ordering_valid = True
for a in non_rejection:
    for b in non_rejection:
        expected = a if order[a] < order[b] else b
        if ALL(a, b) != expected:
            ordering_valid = False
print(f"SC > SP > NP > NN ordering valid for ALL: {ordering_valid}")

# 4. Compute the full 6x6 ANY table
any_table = {a: {b: ANY(a, b) for b in states} for a in states}
print("\nANY Table:")
print("    " + "  ".join(states))
for a in states:
    print(f"{a:<4}" + "  ".join(any_table[a][b] for b in states))

# 5. Check ANY is commutative and idempotent
any_commutative = all(ANY(a, b) == ANY(b, a) for a in states for b in states)
any_idempotent = all(ANY(a, a) == a for a in states)
print(f"\nANY commutative: {any_commutative}")
print(f"ANY idempotent: {any_idempotent}")

# 6. Verify SC is absorbing for ANY
sc_absorbing = all(ANY(a, 'SC') == 'SC' and ANY('SC', a) == 'SC' for a in states)
print(f"SC is absorbing for ANY: {sc_absorbing}")

# 7. Check duality for all 36 pairs
duality_holds = all(ANY(a, b) == dual(ALL(dual(a), dual(b))) for a in states for b in states)
print(f"Duality holds for all 36 pairs: {duality_holds}")
