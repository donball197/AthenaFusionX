// Freestanding WebAssembly Cell for AthenaFusionX
var buffer: [65536]u8 = undefined;

// 1. Export buffer pointer for zero-copy host memory write
pub export fn get_buffer_ptr() [*]u8 {
    return &buffer;
}

// 2. Export kernel execution function (sumBytes)
pub export fn sumBytes(len: i32) f32 {
    var sum: f32 = 0.0;
    const length: usize = @intCast(len);
    for (buffer[0..length]) |b| {
        sum += @floatFromInt(b);
    }
    return sum;
}
