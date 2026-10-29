const std = @import("std");
const core = @import("safetensors.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len < 2) {
        std.debug.print("Usage: {s} <path_to_model.safetensors>\n", .{args[0]});
        return;
    }
    const file_path = args[1];

    const file = std.fs.cwd().openFile(file_path, .{ .mode = .read_only }) catch |err| {
        std.debug.print(">> [ERROR] Could not open file: {any}\n", .{err});
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
    defer std.posix.munmap(mapped);

    var st = try core.SafeTensors.init(mapped, allocator);
    defer st.deinit();

    std.debug.print(">> [INSPECTOR] Zero-Copy Engine Online.\n", .{});
    std.debug.print(">> [INSPECTOR] Reading JSON metadata keys...\n", .{});

    const root_obj = st.parsed.value.object;
    var it = root_obj.iterator();

    var count: usize = 0;
    while (it.next()) |entry| {
        const key = entry.key_ptr.*;
        if (std.mem.eql(u8, key, "__metadata__")) continue;
        std.debug.print("   -> {s}\n", .{key});
        count += 1;
    }

    std.debug.print(">> [INSPECTOR] Found {} total tensors in file.\n", .{count});
}
