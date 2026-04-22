const std = @import("std");

var current_load: f32 = 0.0;

export fn set_substrate_load(load: f32) void {
    current_load = load;
}

export fn extract_thermal_pulse_raw() i32 {
    const temp = (current_load * 40.0) + 30.0;
    return @intFromFloat(temp * 1000.0);
}

export fn extract_reflex(in_ptr: [*]const u8, in_len: usize, out_ptr: [*]u8, out_len: usize) usize {
    _ = in_ptr; _ = in_len; _ = out_ptr; _ = out_len; return 0;
}
