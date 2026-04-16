import os

path = "src/llama_cpu.zig"
print("[*] Wiring Stethoscope into the final gate...")

with open(path, "r") as f:
    data = f.read()

data = data.replace(
    "else => return error.InvalidMetadataType,",
    "else => { std.debug.print(\"\\n[!!!] GATEKEEPER FOUND. ValueType = {any}\\n\", .{value_type}); @panic(\"UNKNOWN_METADATA_TYPE\"); },"
)

with open(path, "w") as f:
    f.write(data)

print("[+] Stethoscope wired.")
