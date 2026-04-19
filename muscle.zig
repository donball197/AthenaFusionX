const std = @import("std");
const posix = std.posix; 

// --- KERNEL STATE ---
var active_vmos: u32 = 0;
var next_handle: u32 = 0xc8;

// Implementation of the VMO creation
export fn zx_vmo_create_native(size: usize) u32 {
    const ptr = posix.mmap(
        null,
        size,
        posix.PROT.READ | posix.PROT.WRITE,
        .{ .TYPE = .PRIVATE, .ANONYMOUS = true },
        -1,
        0,
    ) catch return 0;

    active_vmos += 1;
    return @truncate(@intFromPtr(ptr.ptr));
}

export fn zx_handle_close_native(handle: u32) void {
    const addr: usize = @as(usize, handle);
    
    // ⚡ THE FIX: Explicitly anchor the type as a many-item pointer ([*]u8)
    // before performing the alignment cast to align(4096).
    const raw_ptr = @as([*]u8, @ptrFromInt(addr));
    const ptr: [*]align(4096) u8 = @alignCast(raw_ptr);
    
    posix.munmap(ptr[0..4096]);
    
    if (active_vmos > 0) active_vmos -= 1;
}

// ⚡ Commutation Bridge (Write)
export fn zx_vmo_write_native(handle: u32, buffer: [*]const u8, size: usize) i32 {
    const vmo_ptr: [*]u8 = @ptrFromInt(@as(usize, handle));
    @memcpy(vmo_ptr[0..size], buffer[0..size]);
    return 0; 
}

// ⚡ Commutation Bridge (Read)
export fn zx_vmo_read_native(handle: u32, buffer: [*]u8, size: usize) i32 {
    const vmo_ptr: [*]const u8 = @ptrFromInt(@as(usize, handle));
    @memcpy(buffer[0..size], vmo_ptr[0..size]);
    return 0;
}

export fn muscle_get_active_vmos() u32 { return active_vmos; }
export fn muscle_get_next_handle() u32 { return next_handle; }
