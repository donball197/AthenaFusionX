const std = @import("std");

pub export fn scan_vault(path_ptr: [*]const u8, path_len: usize) void {
    const allocator = std.heap.page_allocator;
    const path = path_ptr[0..path_len];
    var dir = std.fs.cwd().openIterableDir(path, .{}) catch return;
    var iter = dir.iterate();
    while (iter.next() catch null) |entry| {
        std.debug.print("{s}|{s}\n", .{@tagName(entry.kind), entry.name});
    }
}
