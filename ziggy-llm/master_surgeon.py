import re

path = "src/llama_cpu.zig"
print("[*] Initiating Unified Parser Substrate Refactor...")

with open(path, "r") as f:
    code = f.read()

# 1. Clean up lingering syntax errors from our previous Stethoscope probes
code = re.sub(
    r'const value_type = std\.meta\.intToEnum\(ValueType, raw_value_type\) catch @panic\("FRACTURE_LLAMA_ENUM_LINGERING"\); //.*?$',
    r'const value_type = std.meta.intToEnum(ValueType, raw_value_type) catch return error.InvalidMetadataType;',
    code,
    flags=re.MULTILINE
)

# 2. Rewrite readExpectedUnsigned with native Gemma 4 signed integer and array logic
unsigned_replacement = """fn readExpectedUnsigned(comptime T: type, parser: *Parser, value_type: ValueType) !T {
    return switch (value_type) {
        .uint8 => std.math.cast(T, try parser.readInt(u8)) orelse error.InvalidMetadataValue,
        .uint16 => std.math.cast(T, try parser.readInt(u16)) orelse error.InvalidMetadataValue,
        .uint32 => std.math.cast(T, try parser.readInt(u32)) orelse error.InvalidMetadataValue,
        .uint64 => std.math.cast(T, try parser.readInt(u64)) orelse error.InvalidMetadataValue,
        .int8 => std.math.cast(T, try parser.readInt(i8)) orelse return error.InvalidMetadataType,
        .int16 => std.math.cast(T, try parser.readInt(i16)) orelse return error.InvalidMetadataType,
        .int32 => std.math.cast(T, try parser.readInt(i32)) orelse return error.InvalidMetadataType,
        .int64 => std.math.cast(T, try parser.readInt(i64)) orelse return error.InvalidMetadataType,
        .array => {
            const raw_arr_type = try parser.readInt(u32);
            const arr_type = std.meta.intToEnum(ValueType, raw_arr_type) catch return error.InvalidMetadataType;
            const arr_len = try parser.readInt(u64);
            if (arr_len == 0) return 0;
            const first_val = try readExpectedUnsigned(T, parser, arr_type);
            var i: u64 = 1;
            while (i < arr_len) : (i += 1) {
                _ = try readExpectedUnsigned(T, parser, arr_type);
            }
            return first_val;
        },
        else => return error.InvalidMetadataType,
    };
}"""
code = re.sub(r'^fn readExpectedUnsigned\(.*?\).*?^}$', unsigned_replacement, code, flags=re.MULTILINE | re.DOTALL)

# 3. Rewrite readExpectedFloat with its own dedicated, type-safe array handler
float_replacement = """fn readExpectedFloat(parser: *Parser, value_type: ValueType) !f32 {
    return switch (value_type) {
        .float32 => @bitCast(try parser.readInt(u32)),
        .float64 => @floatCast(@as(f64, @bitCast(try parser.readInt(u64)))),
        .array => {
            const raw_arr_type = try parser.readInt(u32);
            const arr_type = std.meta.intToEnum(ValueType, raw_arr_type) catch return error.InvalidMetadataType;
            const arr_len = try parser.readInt(u64);
            if (arr_len == 0) return 0;
            const first_val = try readExpectedFloat(parser, arr_type);
            var i: u64 = 1;
            while (i < arr_len) : (i += 1) {
                _ = try readExpectedFloat(parser, arr_type);
            }
            return first_val;
        },
        else => return error.InvalidMetadataType,
    };
}"""
code = re.sub(r'^fn readExpectedFloat\(.*?\).*?^}$', float_replacement, code, flags=re.MULTILINE | re.DOTALL)

with open(path, "w") as f:
    f.write(code)

print("[+] Unified Parser Substrate successfully integrated.")
