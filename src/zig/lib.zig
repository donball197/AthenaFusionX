const std = @import("std");

var shared_buffer: [4096]u8 = undefined;

export fn get_buffer_ptr() [*]u8 { return &shared_buffer; }
export fn get_buffer_size() usize { return shared_buffer.len; }

export fn calculate_resonance_e2b() f32 {
    var total: f32 = 0.0;
    var i: usize = 0;
    while (i < shared_buffer.len) : (i += 64) {
        const val = @as(f32, @floatFromInt(shared_buffer[i])) / 255.0;
        total += val * val;
    }
    const count = @as(f32, @floatFromInt(shared_buffer.len / 64));
    return if (count > 0) total / count else 0.0;
}

var latent: [32]f32 = [_]f32{0.0} ** 32;
export fn push_latent_intent(input: [*]const f32) void { @memcpy(&latent, input[0..32]); }
export fn get_latent_throttle() f32 {
    var sum: f32 = 0.0;
    for (latent[0..8]) |v| sum += v;
    return sum / 8.0;
}
