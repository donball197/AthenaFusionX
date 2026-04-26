// vision_proc.zig — cleaned up
const std = @import("std");

var latent_intent = [_]f32{0.0} ** 32;

export fn push_latent_intent(input_ptr: [*]const f32) void {
    @memcpy(&latent_intent, input_ptr[0..32]);
}

export fn get_latent_throttle() f32 {
    var sum: f32 = 0.0;
    for (latent_intent[0..8]) |val| sum += val;
    return sum / 8.0;
}

pub export fn calculate_resonance_e2b(mmap_ptr: [*]const u8, length: usize) f32 {
    var total_energy: f32 = 0.0;
    var i: usize = 0;
    while (i < length) : (i += 64) {
        const val = @as(f32, @floatFromInt(mmap_ptr[i])) / 255.0;
        total_energy += val * val;
    }
    const count = @as(f32, @floatFromInt(length / 64));
    return if (count > 0) total_energy / count else 0.0;
}
