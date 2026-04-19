const std = @import("std");

/// Improved thermal probe with dynamic fallback paths for ARM64 
/// (Bridges Crostini Linux and Termux Android environments)
export fn extract_thermal_pulse_raw() i32 {
    const paths = [_][]const u8{
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/hwmon/hwmon0/temp1_input",
        "/sys/class/thermal/thermal_zone1/temp",
        "/proc/acpi/ibm/thermal",
        "/sys/devices/virtual/thermal/thermal_zone0/temp",
    };

    for (paths) |path| {
        const file = std.fs.openFileAbsolute(path, .{}) catch continue;
        defer file.close();
        var buf: [32]u8 = undefined;
        const bytes = file.readAll(&buf) catch continue;
        const slice = std.mem.trim(u8, buf[0..bytes], " \n\r\t");
        return std.fmt.parseInt(i32, slice, 10) catch continue;
    }
    
    // Silent fail -> no crash, allows system monitor to handle the 0.0 state gracefully
    return 0; 
}
