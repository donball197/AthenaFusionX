states = ["SC", "SP", "RC", "RP", "NP", "NN"]

dual_map = {
    "SC": "RC", "RC": "SC",
    "SP": "RP", "RP": "SP",
    "NP": "NP",
    "NN": "NN"
}

def dual(s):
    return dual_map[s]

# ALL table rows = left, cols = right
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

# Verify SC is absorbing for ANY
for s in states:
    assert ANY("SC", s) == "SC", f"ANY(SC, {s}) is {ANY('SC', s)}, expected SC"
    assert ANY(s, "SC") == "SC", f"ANY({s}, SC) is {ANY(s, 'SC')}, expected SC"

print("Verification successful: SC is absorbing for ANY!")
