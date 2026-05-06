import os

cli_path = "src/cli.rs"
with open(cli_path, "r") as f:
    code = f.read()

# Aggressive enforcement of the "No Chatter" rule
old_rules = "=== ABSOLUTE OUTPUT RULES (NEVER VIOLATE) ==="
new_rules = """=== ABSOLUTE OUTPUT RULES (NEVER VIOLATE) ===
- INTERNAL MONOLOGUE IS FORBIDDEN.
- DO NOT explain your reasoning in plain text before the JSON.
- DO NOT "think out loud" about previous observations.
- Your response MUST begin with `[` and end with `]`. 
- Any text outside the JSON array will be treated as a SYSTEM FAILURE."""

code = code.replace(old_rules, new_rules)
with open(cli_path, "w") as f:
    f.write(code)
print("\x1b[32m>> [SUTURE] Gemma silenced. Velocity prioritized.\x1b[0m")
