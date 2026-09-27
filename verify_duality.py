# Define states
SC, SP, RC, RP, NP, NN = 'SC', 'SP', 'RC', 'RP', 'NP', 'NN'
states = [SC, SP, RC, RP, NP, NN]

# Dual mapping
dual_map = {
    SC: RC, RC: SC,
    SP: RP, RP: SP,
    NP: NP,
    NN: NN
}

def dual(s):
    return dual_map[s]

# ALL table
ALL_table = {
    SC: {SC: SC, SP: SP, RC: RC, RP: RP, NP: NP, NN: NN},
    SP: {SC: SP, SP: SP, RC: RC, RP: RP, NP: NP, NN: NN},
    RC: {SC: RC, SP: RC, RC: RC, RP: RC, NP: RC, NN: RC},
    RP: {SC: RP, SP: RP, RC: RC, RP: RP, NP: RP, NN: RP},
    NP: {SC: NP, SP: NP, RC: RC, RP: RP, NP: NP, NN: NN},
    NN: {SC: NN, SP: NN, RC: RC, RP: RP, NP: NN, NN: NN}
}

def ALL(a, b):
    return ALL_table[a][b]

# ANY derived via duality
def ANY(a, b):
    return dual(ALL(dual(a), dual(b)))

# 1. Verify ALL is commutative and idempotent
for a in states:
    assert ALL(a, a) == a, f"ALL is not idempotent for {a}"
    for b in states:
        assert ALL(a, b) == ALL(b, a), f"ALL is not commutative for {a}, {b}"

# 2. Verify RC is absorbing for ALL
for a in states:
    assert ALL(a, RC) == RC, f"RC is not absorbing for ALL with {a}"
    assert ALL(RC, a) == RC, f"RC is not absorbing for ALL with {a}"

# 3. Verify SC > SP > NP > NN ordering for non-rejection states
order = {SC: 4, SP: 3, NP: 2, NN: 1}
non_rejection = [SC, SP, NP, NN]
for a in non_rejection:
    for b in non_rejection:
        expected = a if order[a] < order[b] else b
        assert ALL(a, b) == expected, f"Ordering failed for ALL({a}, {b}): expected {expected}, got {ALL(a, b)}"

# 4. Compute the full 6x6 ANY table
ANY_table = {}
for a in states:
    ANY_table[a] = {}
    for b in states:
        ANY_table[a][b] = ANY(a, b)

# 5. Check ANY is commutative and idempotent
for a in states:
    assert ANY(a, a) == a, f"ANY is not idempotent for {a}"
    for b in states:
        assert ANY(a, b) == ANY(b, a), f"ANY is not commutative for {a}, {b}"

# 6. Verify SC is absorbing for ANY
for a in states:
    assert ANY(a, SC) == SC, f"SC is not absorbing for ANY with {a}"
    assert ANY(SC, a) == SC, f"SC is not absorbing for ANY with {a}"

# 7. Check duality for all 36 pairs
for a in states:
    for b in states:
        assert ANY(a, b) == dual(ALL(dual(a), dual(b))), f"Duality failed for {a}, {b}"

print("All checks passed successfully!")
