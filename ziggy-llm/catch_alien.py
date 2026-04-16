import os

path = "src/llama_cpu.zig"
print("[*] Wiring Stethoscope into Tensor Type gateway...")

with open(path, "r") as f:
    code = f.read()

target = "const tensor_type = std.meta.intToEnum(TensorType, raw_type) catch return error.UnsupportedTensorType;"
injection = """const tensor_type = std.meta.intToEnum(TensorType, raw_type) catch {
        std.debug.print("\\n[!!!] ALIEN TENSOR TYPE ID: {d} on tensor: {s}\\n", .{raw_type, name});
        @panic("ALIEN_TENSOR");
    };"""

if target in code:
    code = code.replace(target, injection)
    with open(path, "w") as f:
        f.write(code)
    print("[+] Stethoscope wired successfully.")
else:
    print("[-] Target not found. Verify file state.")
