import os

path = "src/llama_cpu.zig"
print("[*] Re-wiring Q5_K calibration enum...")

with open(path, "r") as f:
    code = f.read()

# Swap the guessed enum for a valid calibration reference
code = code.replace(
    ".q5_k => .legacy_q5_k,",
    ".q5_k => .f32_reference,"
)

with open(path, "w") as f:
    f.write(code)

print("[+] Q5_K calibration safely mapped.")
