import os

path = "src/self_healing_engine.rs"
print("[*] Synchronizing Self-Healing Engine...")

with open(path, "r") as f:
    code = f.read()

# Fix the multi-line call to remember()
code = code.replace(".store_memory(", ".remember(")
# Ensure the closing of the call includes the 0.0 credits
code = code.replace("&resolution\n        ).await?", "&resolution, 0.0\n        ).await?")

with open(path, "w") as f:
    f.write(code)

print("[+] Self-Healing Engine aligned.")
