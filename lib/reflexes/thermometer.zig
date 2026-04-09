const std = @import("std");
const fs = std.fs;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const allocator = gpa.allocator();
    defer _ = gpa.deinit();

    var dir = try fs.cwd().openDir("/sys/class/hwmon/", .{ .iterate = true });
    defer dir.close();
    var iter = dir.iterate();

    while (try iter.next()) |entry| {
        if (std.mem.startsWith(u8, entry.name, "hwmon")) {
            const path = try std.fmt.allocPrint(allocator, "/sys/class/hwmon/{s}/temp1_input", .{entry.name});
            defer allocator.free(path);

            const file = fs.openFileAbsolute(path, .{}) catch continue;
            defer file.close();

            var buf: [16]u8 = undefined;
            const n = try file.readAll(&buf);
            if (n == 0) continue;

            const val_str = std.mem.trim(u8, buf[0..n], "\n\r");
            const millicelsius = try std.fmt.parseInt(i32, val_str, 10);
            
            std.debug.print("{d:.1}\n", .{@as(f32, @floatFromInt(millicelsius)) / 1000.0});
            return;
        }
    }
    std.debug.print("ERROR: NO_HW_MONITOR\n", { });
}
