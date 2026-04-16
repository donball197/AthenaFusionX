import os

path = "src/gguf.zig"
with open(path, "r") as f:
    code = f.read()

# Locate the exact moment the ID card reads the key and type
target = "const raw_value_type = readInt(reader, u32) catch return error.TruncatedFile;"
injection = "const raw_value_type = readInt(reader, u32) catch return error.TruncatedFile;\n    std.debug.print(\"[STETHOSCOPE] Key: {s} | TypeID: {d}\\n\", .{key, raw_value_type});"

# Inject only into parseMetadataEntry
if "[STETHOSCOPE]" not in code:
    code = code.replace(target, injection, 1)
    with open(path, "w") as f:
        f.write(code)
    print("[+] Stethoscope successfully wired to the substrate.")
else:
    print("[*] Stethoscope already attached.")
