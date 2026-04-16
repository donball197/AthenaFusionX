import re

path = "src/llama_cpu.zig"
print("[*] Assimilating BF16 (Brain Float) Tensor Type...")

with open(path, "r") as f:
    code = f.read()

# 1. Add bf16 (ID 30) to TensorType enum
code = code.replace(
    """pub const TensorType = enum(u32) {
    f32 = 0,
    f16 = 1,
    q8_0 = 8,
    q4_k = 12,
    q6_k = 14,""",
    """pub const TensorType = enum(u32) {
    f32 = 0,
    f16 = 1,
    q8_0 = 8,
    q4_k = 12,
    q6_k = 14,
    bf16 = 30,"""
)

# 2. Tell the memory allocator bf16 is 2 bytes wide (same as f16)
code = code.replace(
    ".f16 => try std.math.mul(usize, row_len, 2),",
    ".f16 => try std.math.mul(usize, row_len, 2),\n        .bf16 => try std.math.mul(usize, row_len, 2),"
)

# 3. Add bf16 to the dequantizeRow mapping
code = code.replace(
    """        .f16 => {
            for (0..row_len) |index| out[index] = readF16AsF32(row[index * 2 ..][0..2]);
        },""",
    """        .f16 => {
            for (0..row_len) |index| out[index] = readF16AsF32(row[index * 2 ..][0..2]);
        },
        .bf16 => {
            for (0..row_len) |index| out[index] = readBF16AsF32(row[index * 2 ..][0..2]);
        },"""
)

# 4. Inject the BF16 -> F32 bit-shift converter
bf16_converter = """
fn readBF16AsF32(bytes: []const u8) f32 {
    const raw = std.mem.readInt(u16, bytes[0..2], .little);
    // BF16 is just the top 16 bits of an F32. Shift it up.
    const f32_bits = @as(u32, raw) << 16;
    return @bitCast(f32_bits);
}
"""
code = code.replace(
    "fn readF16AsF32(bytes: []const u8) f32 {",
    bf16_converter + "\nfn readF16AsF32(bytes: []const u8) f32 {"
)

with open(path, "w") as f:
    f.write(code)

print("[+] BF16 Substrate mapped and integrated.")
