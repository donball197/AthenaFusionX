import os

fpath = "src/orchestrator.rs"
with open(fpath, "r") as f:
    code = f.read()

# 1. Add Nucleus Sampling (topK / topP) to break the probability loop
if '"topK"' not in code:
    code = code.replace(
        '"temperature": 0.6,',
        '"temperature": 0.65,\n                "topK": 40,\n                "topP": 0.90,'
    )

# 2. Add an explicit length constraint to the reasoning field to stop rambling
if 'ANTI-LOOP PROTOCOL' not in code:
    code = code.replace(
        'combined_text.push_str(&prompt);',
        'combined_text.push_str("\\n[ANTI-LOOP PROTOCOL]: You must keep the \\"reasoning\\" string strictly under 2 sentences to prevent cognitive loops.\\n");\n        combined_text.push_str(&prompt);'
    )

with open(fpath, "w") as f:
    f.write(code)

print(">> [SUTURE] Nucleus sampling and cognitive bounds injected into Orchestrator.")
