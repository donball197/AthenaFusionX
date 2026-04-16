import os

print("[*] Assimilating Q5_K (ID 13) into Universal Substrate...")

# 1. Forge the Q5_K muscles into llama_cpu.zig
path = "src/llama_cpu.zig"
with open(path, "r") as f:
    code = f.read()

if "q5_k = 13," not in code:
    # Map the enum
    code = code.replace("q4_k = 12,", "q4_k = 12,\n    q5_k = 13,")
    
    # Map the memory stride (176 bytes per 256 weights)
    code = code.replace(".q4_k => blk: {", ".q5_k => blk: {\n            if (row_len % 256 != 0) return error.InvalidTensorMetadata;\n            break :blk try std.math.mul(usize, row_len / 256, 176);\n        },\n        .q4_k => blk: {")
    
    # Map the switch statements
    code = code.replace(".q4_k => try dequantizeRowQ4K(out, row, row_len),", ".q4_k => try dequantizeRowQ4K(out, row, row_len),\n        .q5_k => try dequantizeRowQ5K(out, row, row_len),")
    code = code.replace(".q4_k => try dotQ4KRow(row, row_len, input),", ".q4_k => try dotQ4KRow(row, row_len, input),\n        .q5_k => try dotQ5KRow(row, row_len, input),")
    code = code.replace(".q4_k => dotQ4KRow(row, row_len, input) catch unreachable,", ".q4_k => dotQ4KRow(row, row_len, input) catch unreachable,\n        .q5_k => dotQ5KRow(row, row_len, input) catch unreachable,")

    # Inject the raw math primitives at the bottom of the file
    q5k_funcs = """
fn dequantizeRowQ5K(out: []f32, row: []const u8, row_len: usize) !void {
    if (row_len % 256 != 0) return error.InvalidTensorMetadata;
    var out_index: usize = 0;
    var block_index: usize = 0;
    while (block_index < row.len) : (block_index += 176) {
        const block = row[block_index .. block_index + 176];
        const d = readF16AsF32(block[0..2]);
        const dmin = readF16AsF32(block[2..4]);
        const scales: []const u8 = block[4..16];
        const qh: []const u8 = block[16..48];
        var q: []const u8 = block[48..176];
        var scale_index: usize = 0;

        var group: usize = 0;
        while (group < 4) : (group += 1) {
            const sm0 = getScaleMinK4(scale_index + 0, scales);
            const sm1 = getScaleMinK4(scale_index + 1, scales);
            const d1 = d * @as(f32, @floatFromInt(sm0.scale));
            const m1 = dmin * @as(f32, @floatFromInt(sm0.min));
            const d2 = d * @as(f32, @floatFromInt(sm1.scale));
            const m2 = dmin * @as(f32, @floatFromInt(sm1.min));

            for (0..32) |l| {
                const h_bit = @as(u8, (qh[l] >> @intCast(group)) & 1);
                const q_val = (q[l] & 0x0F) | (h_bit << 4);
                out[out_index + l] = d1 * @as(f32, @floatFromInt(q_val)) - m1;
            }
            for (0..32) |l| {
                const h_bit = @as(u8, (qh[l] >> @intCast(group + 4)) & 1);
                const q_val = (q[l] >> 4) | (h_bit << 4);
                out[out_index + 32 + l] = d2 * @as(f32, @floatFromInt(q_val)) - m2;
            }
            out_index += 64;
            q = q[32..];
            scale_index += 2;
        }
    }
}

fn dotQ5KRow(row: []const u8, row_len: usize, input: []const f32) !f32 {
    if (row_len % 256 != 0) return error.InvalidTensorMetadata;
    var sum: f32 = 0;
    var input_offset: usize = 0;
    var block_index: usize = 0;
    while (block_index < row.len) : (block_index += 176) {
        const block = row[block_index .. block_index + 176];
        const d = readF16AsF32(block[0..2]);
        const dmin = readF16AsF32(block[2..4]);
        const scales: []const u8 = block[4..16];
        const qh: []const u8 = block[16..48];
        var q: []const u8 = block[48..176];
        var scale_index: usize = 0;

        var group: usize = 0;
        while (group < 4) : (group += 1) {
            const sm0 = getScaleMinK4(scale_index + 0, scales);
            const sm1 = getScaleMinK4(scale_index + 1, scales);
            const d1 = d * @as(f32, @floatFromInt(sm0.scale));
            const m1 = dmin * @as(f32, @floatFromInt(sm0.min));
            const d2 = d * @as(f32, @floatFromInt(sm1.scale));
            const m2 = dmin * @as(f32, @floatFromInt(sm1.min));

            for (0..32) |l| {
                const h_bit = @as(u8, (qh[l] >> @intCast(group)) & 1);
                const q_val = (q[l] & 0x0F) | (h_bit << 4);
                const w = d1 * @as(f32, @floatFromInt(q_val)) - m1;
                sum += w * input[input_offset + l];
            }
            for (0..32) |l| {
                const h_bit = @as(u8, (qh[l] >> @intCast(group + 4)) & 1);
                const q_val = (q[l] >> 4) | (h_bit << 4);
                const w = d2 * @as(f32, @floatFromInt(q_val)) - m2;
                sum += w * input[input_offset + 32 + l];
            }
            input_offset += 64;
            q = q[32..];
            scale_index += 2;
        }
    }
    return sum;
}
"""
    code += q5k_funcs
    with open(path, "w") as f:
        f.write(code)

# 2. Patch ziggy_format.zig serialization compliance
path = "src/ziggy_format.zig"
with open(path, "r") as f:
    code = f.read()
if ".q5_k => .generic_quant_raw," not in code:
    code = code.replace(".q4_k => if (options.repack_q4_k) .q4_k_m_packed else .generic_quant_raw,", ".q4_k => if (options.repack_q4_k) .q4_k_m_packed else .generic_quant_raw,\n        .q5_k => .generic_quant_raw,")
    with open(path, "w") as f:
        f.write(code)

# 3. Patch llama_fixture.zig compiler compliance
path = "src/runtime/llama_fixture.zig"
with open(path, "r") as f:
    code = f.read()
if ".q5_k => unreachable," not in code:
    code = code.replace(".q4_k => try writeTensorDataQ4K(list, values, row_len),", ".q4_k => try writeTensorDataQ4K(list, values, row_len),\n        .q5_k => unreachable,")
    with open(path, "w") as f:
        f.write(code)

print("[+] Q5_K muscles fully forged into engine.")
