states = ['SC', 'SP', 'RC', 'RP', 'NP', 'NN']

dual_map = {
    'SC': 'RC', 'RC': 'SC',
    'SP': 'RP', 'RP': 'SP',
    'NP': 'NP',
    'NN': 'NN'
}

def dual(s):
    return dual_map[s]

# ALL table (rows = left, cols = right)
all_matrix = {
    'SC': {'SC': 'SC', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'SP': {'SC': 'SP', 'SP': 'SP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'RC': {'SC': 'RC', 'SP': 'RC', 'RC': 'RC', 'RP': 'RC', 'NP': 'RC', 'NN': 'RC'},
    'RP': {'SC': 'RP', 'SP': 'RP', 'RC': 'RC', 'RP': 'RP', 'NP': 'RP', 'NN': 'RP'},
    'NP': {'SC': 'NP', 'SP': 'NP', 'RC': 'RC', 'RP': 'RP', 'NP': 'NP', 'NN': 'NN'},
    'NN': {'SC': 'NN', 'SP': 'NN', 'RC': 'RC', 'RP': 'RP', 'NP': 'NN', 'NN': 'NN'}
}

def ALL(a, b):
    return all_matrix[a][b]

def ANY(a, b):
    return dual(ALL(dual(a), dual(b)))

# 1. Verify ALL is commutative and idempotent
all_commutative = all(ALL(a, b) == ALL(b, a) for a in states for b in states)
all_idempotent = all(ALL(a, a) == a for a in states)

# 2. Verify RC is absorbing for ALL
rc_absorbing = all(ALL('RC', a) == 'RC' and ALL(a, 'RC') == 'RC' for a in states)

# 3. Verify SC > SP > NP > NN ordering for non-rejection states
order = {'SC': 4, 'SP': 3, 'NP': 2, 'NN': 1}
non_rejection = ['SC', 'SP', 'NP', 'NN']
ordering_valid = True
for a in non_rejection:
    for b in non_rejection:
        expected_val = a if order[a] < order[b] else b
        if ALL(a, b) != expected_val:
            ordering_valid = False

# 4. Compute the full 6x6 ANY table
any_matrix = {}
for a in states:
    any_matrix[a] = {}
    for b in states:
        any_matrix[a][b] = ANY(a, b)

# 5. Check ANY is commutative and idempotent
any_commutative = all(ANY(a, b) == ANY(b, a) for a in states for b in states)
any_idempotent = all(ANY(a, a) == a for a in states)

# 6. Verify SC is absorbing for ANY
sc_absorbing = all(ANY('SC', a) == 'SC' and ANY(a, 'SC') == 'SC' for a in states)

# 7. Check duality for all 36 pairs
duality_valid = True
for a in states:
    for b in states:
        if ANY(a, b) != dual(ALL(dual(a), dual(b))):
            duality_valid = False

print(f"ALL_COMMUTATIVE: {all_commutative}")
print(f"ALL_IDEMPOTENT: {all_idempotent}")
print(f"RC_ABSORBING: {rc_absorbing}")
print(f"ORDERING_VALID: {ordering_valid}")
print(f"ANY_COMMUTATIVE: {any_commutative}")
print(f"ANY_IDEMPOTENT: {any_idempotent}")
print(f"SC_ABSORBING: {sc_absorbing}")
print(f"DUALITY_VALID: {duality_valid}")

print("\nANY Table:")
print("    " + "  ".join(states))
for a in states:
    row = [any_matrix[a][b] for b in states]
    print(f"{a}  " + "  ".join(row))
