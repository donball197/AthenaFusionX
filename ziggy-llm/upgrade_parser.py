import os

print("[*] Initiating Sovereign Parser Upgrade...")

# 1. Upgrade the GGUF Parser
gguf_path = "src/gguf.zig"
with open(gguf_path, "r") as f:
    gguf = f.read()

old_func = """fn readExpectedUnsigned(comptime T: type, reader: anytype, value_type: ValueType) !T {
    return switch (value_type) {
        .uint8 => try castUnsigned(T, readInt(reader, u8) catch return error.TruncatedFile),
        .uint16 => try castUnsigned(T, readInt(reader, u16) catch return error.TruncatedFile),
        .uint32 => try castUnsigned(T, readInt(reader, u32) catch return error.TruncatedFile),
        .uint64 => try castUnsigned(T, readInt(reader, u64) catch return error.TruncatedFile),
        else => error.InvalidMetadataType,
    };
}"""

new_func = """fn readExpectedUnsigned(comptime T: type, reader: anytype, value_type: ValueType) !T {
    return switch (value_type) {
        .uint8 => try castUnsigned(T, readInt(reader, u8) catch return error.TruncatedFile),
        .int8 => try castUnsigned(T, readInt(reader, i8) catch return error.TruncatedFile),
        .uint16 => try castUnsigned(T, readInt(reader, u16) catch return error.TruncatedFile),
        .int16 => try castUnsigned(T, readInt(reader, i16) catch return error.TruncatedFile),
        .uint32 => try castUnsigned(T, readInt(reader, u32) catch return error.TruncatedFile),
        .int32 => try castUnsigned(T, readInt(reader, i32) catch return error.TruncatedFile),
        .uint64 => try castUnsigned(T, readInt(reader, u64) catch return error.TruncatedFile),
        .int64 => try castUnsigned(T, readInt(reader, i64) catch return error.TruncatedFile),
        else => error.InvalidMetadataType,
    };
}"""

if old_func in gguf:
    gguf = gguf.replace(old_func, new_func)
    with open(gguf_path, "w") as f:
        f.write(gguf)
    print("[+] src/gguf.zig integer casting pathways upgraded.")
else:
    print("[!] Warning: old_func not found. Parser may already be patched.")

# 2. Silence the Update Pulse
main_path = "src/main.zig"
if os.path.exists(main_path):
    with open(main_path, "r") as f:
        main = f.read()
    
    if "update.checkForUpdates(allocator);" in main:
        main = main.replace("update.checkForUpdates(allocator);", "// update.checkForUpdates(allocator);")
        with open(main_path, "w") as f:
            f.write(main)
        print("[+] src/main.zig update pulse silenced.")

print("[*] Upgrade Complete.")
