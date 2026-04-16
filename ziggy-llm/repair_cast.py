import os

path = "src/llama_cpu.zig"
print("[*] Re-wiring integer casts to standard library...")

with open(path, "r") as f:
    data = f.read()

# Replace the ghost helper function with native std.math.cast
data = data.replace(
    ".int8 => try castUnsigned(T, try parser.readInt(u8)),",
    ".int8 => std.math.cast(T, try parser.readInt(u8)) orelse return error.InvalidMetadataType,"
)
data = data.replace(
    ".int16 => try castUnsigned(T, try parser.readInt(u16)),",
    ".int16 => std.math.cast(T, try parser.readInt(u16)) orelse return error.InvalidMetadataType,"
)
data = data.replace(
    ".int32 => try castUnsigned(T, try parser.readInt(u32)),",
    ".int32 => std.math.cast(T, try parser.readInt(u32)) orelse return error.InvalidMetadataType,"
)
data = data.replace(
    ".int64 => try castUnsigned(T, try parser.readInt(u64)),",
    ".int64 => std.math.cast(T, try parser.readInt(u64)) orelse return error.InvalidMetadataType,"
)

with open(path, "w") as f:
    f.write(data)

print("[+] Casts re-wired. Muscle is structurally sound.")
