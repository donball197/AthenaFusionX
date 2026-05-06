import os

fpath = "src/cli.rs"
if os.path.exists(fpath):
    with open(fpath, "r") as f:
        code = f.read()

    # SUTURE: Forbid early halting
    new_law = """=== STRICT OUTPUT RULES ===
- You MUST respond with **ONLY** a valid JSON array of AgentAction objects. 
- Nothing else: no reasoning, no explanations, no markdown, no thoughts.
- Response must start with `[` and end with `]`.
- Every action must have a "reasoning" field.
- DO NOT use the `halt` action just to "wait" for the result of a `read_file` or `execute_command`. The system automatically provides the result in the next turn.
- ONLY use `halt` when the ENTIRE mission is 100% complete."""

    code = code.replace(
        "=== STRICT OUTPUT RULES ===\n- You MUST respond with **ONLY** a valid JSON array of AgentAction objects. \n- Nothing else: no reasoning, no explanations, no markdown, no thoughts.\n- Response must start with `[` and end with `]`.\n- Every action must have a \"reasoning\" field.",
        new_law
    )
    
    with open(fpath, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] Execution Law hardened against early halting.\x1b[0m")
