const std = @import("std");

/// Pulls live thermal data from the kernel (ARM64 safe)
export fn get_thermal_temp() callconv(.C) f32 {
    const path = "/sys/class/thermal/thermal_zone0/temp";
    var file = std.fs.openFileAbsolute(path, .{}) catch return 0.0;
    defer file.close();

    var buf: [16]u8 = undefined;
    const amt = file.readAll(&buf) catch return 0.0;
    if (amt == 0) return 0.0;
    
    const temp_str = std.mem.trim(u8, buf[0..amt], " \n\t\r");
    const temp_raw = std.fmt.parseFloat(f32, temp_str) catch return 0.0;
    
    return temp_raw / 1000.0;
}

/// Hardware bind success stub (always returns true for current ARM64 stability)
export fn resonator_ignite(path_ptr: [*]const u8, path_len: usize) callconv(.C) bool {
    _ = path_ptr;
    _ = path_len;
    return true; 
}

export fn scan_entropy() callconv(.C) f32 {
    return 0.95; // High entropy baseline for NEXUS resonance
}
