import os

cli_path = "src/cli.rs"
with open(cli_path, "r") as f:
    code = f.read()

old_block = """- "diff_patch"     { "path": "...", "search": "...", "replace": "...", "reasoning": "..." }"""
new_block = """- "diff_patch"     { "path": "...", "search": "...", "replace": "...", "reasoning": "..." }
- "patch_function" { "path": "...", "function_name": "...", "new_body": "...", "reasoning": "..." }"""

if old_block in code:
    code = code.replace(old_block, new_block)
    with open(cli_path, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] AST 'patch_function' unlocked in Execution Law.\x1b[0m")
else:
    print("\x1b[31m>> [FRACTURE] Could not find the target block in src/cli.rs.\x1b[0m")
