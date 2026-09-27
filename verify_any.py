states = ["SC", "SP", "RC", "RP", "NP", "NN"]

any_table = {
    "SC": {"SC": "SC", "SP": "SC", "RC": "SC", "RP": "SC", "NP": "SC", "NN": "SC"},
    "SP": {"SC": "SC", "SP": "SP", "RC": "SP", "RP": "SP", "NP": "SP", "NN": "SP"},
    "RC": {"SC": "SC", "SP": "SP", "RC": "RC", "RP": "RP", "NP": "NP", "NN": "NN"},
    "RP": {"SC": "SC", "SP": "SP", "RC": "RP", "RP": "RP", "NP": "NP", "NN": "NN"},
    "NP": {"SC": "SC", "SP": "SP", "RC": "NP", "RP": "NP", "NP": "NP", "NN": "NN"},
    "NN": {"SC": "SC", "SP": "SP", "RC": "NN", "RP": "NN", "NP": "NN", "NN": "NN"},
}

# Check commutativity
commutative = True
for a in states:
    for b in states:
        if any_table[a][b] != any_table[b][a]:
            print(f"Commutativity failed for {a}, {b}: ANY({a},{b})={any_table[a][b]}, ANY({b},{a})={any_table[b][a]}")
            commutative = False

# Check idempotency
idempotent = True
for a in states:
    if any_table[a][a] != a:
        print(f"Idempotency failed for {a}: ANY({a},{a})={any_table[a][a]}")
        idempotent = False

if commutative and idempotent:
    print("ANY table is commutative and idempotent!")
else:
    print("Verification failed.")
