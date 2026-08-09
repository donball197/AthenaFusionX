import os

print(">> Upgrading JSON Translator and Registering Kernel Module...")

path = "crates/runtime_core/src/epistemic_engine.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    # The old JSON prompt instruction
    old_prompt = """             Enclose in a JSON array. Do not output anything else.\","""
    
    # The new prompt that forces strict escaping
    new_prompt = """             CRITICAL JSON ESCAPING RULE: You MUST escape all internal newlines inside strings as \\n. NEVER put literal, raw newlines inside a JSON string value.
             Enclose in a JSON array. Do not output anything else.\","""

    if old_prompt in content:
        content = content.replace(old_prompt, new_prompt)
        with open(path, "w") as f:
            f.write(content)
        print("[OK] EpistemicEngine Patched: Strict JSON Escaping Active.")
    else:
        print("[!] JSON Prompt target not found. May already be patched.")

# Register safe_buffer in the kernel
mod_path = "crates/nexus_substrate/src/kernel/mod.rs"
if os.path.exists(mod_path):
    with open(mod_path, "r") as f:
        mod_content = f.read()
    
    old_mod = """pub mod safe_vmo;
pub mod temp_vmo;"""
    
    new_mod = """pub mod safe_vmo;
pub mod temp_vmo;
pub mod safe_buffer;"""

    if old_mod in mod_content and "pub mod safe_buffer;" not in mod_content:
        mod_content = mod_content.replace(old_mod, new_mod)
        with open(mod_path, "w") as f:
            f.write(mod_content)
        print(f"[OK] Patched {mod_path} - safe_buffer module registered.")
    else:
        print(f"[!] Could not find target in {mod_path}. May already be registered.")

print(">> Cortex Upgrade Complete.")
