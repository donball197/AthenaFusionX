#!/bin/bash
# forge_diagnostic.sh - Project Nexus Singularity v0.2.2

echo ">> [NEXUS] Patching Zig 0.15 Alignment Strictness..."
sed -i 's/std.posix.munmap(ctx.mapped);/std.posix.munmap(@alignCast(ctx.mapped));/g' lib/nexus_loader.zig

echo ">> [NEXUS] Recompiling libnexus_loader.so..."
zig build-lib lib/nexus_loader.zig -O ReleaseSmall -dynamic -fPIC -femit-bin=lib/libnexus_loader.so

echo ">> [NEXUS] Forging BF16 Text Decoder Diagnostic Mapper..."
cat << 'INNER' > lib/inspect_bf16.zig
const std = @import("std");
const core = @import("safetensors.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();

    const file_path = "/mnt/chromeos/removable/Memorex USB/AthenaFusionX/gemma-4-e2b-weights/model.safetensors";
    const file = std.fs.cwd().openFile(file_path, .{ .mode = .read_only }) catch |err| {
        std.debug.print(">> [ERROR] Could not open file: {}\n", .{err});
        return;
    };
    defer file.close();

    const mapped = try std.posix.mmap(
        null,
        (try file.stat()).size,
        std.posix.PROT.READ,
        .{ .TYPE = .SHARED },
        file.handle,
        0,
    );
    defer std.posix.munmap(@alignCast(mapped));

    var st = try core.SafeTensors.init(mapped, allocator);
    defer st.deinit();

    std.debug.print(">> [DIAGNOSTIC] Scanning for BF16 Text Decoder Layers...\n", .{});
    const root = st.parsed.value.object;
    var it = root.iterator();
    var count: usize = 0;

    while (it.next()) |entry| {
        const key = entry.key_ptr.*;
        if (std.mem.eql(u8, key, "__metadata__")) continue;

        const obj = entry.value_ptr.object;
        const dtype = obj.get("dtype").?.string;
        
        // Target specifically BF16 weights in the core transformer layers
        if (std.mem.eql(u8, dtype, "BF16")) {
            if (std.mem.indexOf(u8, key, "model.layers.0.") != null) {
                const shape = obj.get("shape").?.array;
                std.debug.print("   -> {s} | Shape: [", .{key});
                for (shape.items, 0..) |dim, i| {
                    if (i > 0) std.debug.print(", ", .{});
                    std.debug.print("{}", .{dim.integer});
                }
                std.debug.print("]\n", .{});
                count += 1;
            }
        }
    }
    std.debug.print(">> [DIAGNOSTIC] Extracted {} BF16 base layer signatures.\n", .{count});
}
INNER

echo ">> [NEXUS] Compiling diagnostic executable..."
zig build-exe lib/inspect_bf16.zig -O ReleaseFast -femit-bin=inspect_bf16

echo ">> [NEXUS] Complete. Ready to execute."
