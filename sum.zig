var buffer: [65536]u8 = undefined;

export fn get_buffer_ptr() [*]u8 {
    return &buffer;
}

export fn sumBytes(len: i32) f32 {
    var total: u32 = 0;
    var i: usize = 0;
    const ulen = @as(usize, @intCast(len));
    while (i < ulen) : (i += 1) {
        total += buffer[i];
    }
    return @as(f32, @floatFromInt(total));
}
