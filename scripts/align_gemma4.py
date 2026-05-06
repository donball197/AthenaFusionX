import os

fpath = "src/router.rs"
if not os.path.exists(fpath):
    print(">> [FAULT] src/router.rs not found.")
    exit(1)

with open(fpath, "r") as f:
    code = f.read()

# 1. Strip the XML tag instructions from the final prompt builder
if "NEXT STEP (Output <THOUGHT> then <ACTION> JSON array):" in code:
    code = code.replace(
        "NEXT STEP (Output <THOUGHT> then <ACTION> JSON array):",
        "NEXT STEP (Output ONLY a valid JSON array):"
    )

# 2. Align the personas to stop using <ACTION> tags
code = code.replace(
    "RULE: You MUST output a valid JSON array inside an <ACTION> block.",
    "RULE: You MUST output ONLY a valid JSON array."
)
code = code.replace(
    "RULE: You MUST execute operations using the <ACTION> JSON array.",
    "RULE: You MUST execute operations using ONLY a valid JSON array where every object has a 'type' field."
)
code = code.replace(
    "NEVER output conversational text outside of <THOUGHT>.",
    "NEVER output conversational text. Output ONLY JSON."
)

with open(fpath, "w") as f:
    f.write(code)

print(">> [SUTURE] src/router.rs aligned for Gemma 4 structured outputs.")
