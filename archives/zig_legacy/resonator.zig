/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


const std = @import("std");

/// The 32-D Latent Space Proxy
/// [0..7]   - Power/Thermal Intent (Throttle)
/// [8..15]  - Neural Attention Priority
/// [16..31] - Spatiotemporal Action Vectors
var latent_intent = [_]f32{0.0} ** 32;

/// Push: Rust calls this to send the AI's "Imaginative" intent to the NPU
export fn push_latent_intent(input_ptr: [*]const f32) void {
    const data: [*]const f32 = input_ptr;
    inline for (0..32) |i| {
        latent_intent[i] = data[i];
    }
}

/// Pull: Rust calls this to check the "Reflex" hardware status
export fn get_latent_throttle() f32 {
    var sum: f32 = 0.0;
    // Calculate a safety floor based on the first 8 dimensions
    for (latent_intent[0..8]) |val| {
        sum += val;
    }
    return sum / 8.0;
}

/// Original Resonance Logic for Hardware Monitoring
pub export fn calculate_resonance_e2b(mmap_ptr: [*]const u8, length: usize) f32 {
    var total_energy: f32 = 0.0;
    var i: usize = 0;
    while (i < length) : (i += 64) {
        const byte = mmap_ptr[i];
        const val = (@as(f32, @floatFromInt(byte)) / 255.0);
        total_energy += (val * val);
    }
    const count = @as(f32, @floatFromInt(length / 64));
    return if (count > 0) total_energy / count else 0.0;
}
