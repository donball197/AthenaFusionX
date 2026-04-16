import re
import os

path = "src/llama_cpu.zig"
print(f"[*] Initiating Deep Substrate Healing on {path}...")

with open(path, "r") as f:
    data = f.read()

# 1. Target the broken multi-line catch blocks and excise them
pattern = r'catch \|err\| \{ std\.debug\.print\(".*?(?:\", \.\{raw_value_type\}\); return err; \};|return err; \};)'
data = re.sub(pattern, 'catch @panic("FRACTURE_LLAMA_ENUM_DEEP");', data, flags=re.DOTALL)

# 2. Catch any lingering open strings from the previous sed
data = data.replace(
    'catch |err| { std.debug.print("', 
    'catch @panic("FRACTURE_LLAMA_ENUM_LINGERING"); // '
)

with open(path, "w") as f:
    f.write(data)

print("[+] Muscle successfully healed and traps re-armed.")
