const std = @import("std");
const posix = std.posix;

const MAX_VMOS = 1024;
const PAGE_SIZE = 4096;
var active_vmos: u32 = 0;
var next_handle: u32 = 1;

// Registry to track allocation sizes and pointers for clean unmapping
var handle_sizes: [MAX_VMOS]usize = [_]usize{0} ** MAX_VMOS;
var handle_ptrs: [MAX_VMOS][*]align(PAGE_SIZE) u8 = undefined;

pub const VmoInfo = extern struct {
    handle: u32,
    size: usize,
    committed_pages: usize,
    is_cow: u8,
    parent_handle: u32,
};

export fn zx_vmo_create_native(size: usize) u32 {
    if (next_handle >= MAX_VMOS) return 0;
    
    // Zig 0.15 strictly requires MAP to be passed as a packed struct on ARM64
    const flags = posix.MAP{ .TYPE = .PRIVATE, .ANONYMOUS = true };
    
    const ptr = posix.mmap(null, size, posix.PROT.READ | posix.PROT.WRITE, flags, -1, 0) catch return 0;
    
    const h = next_handle;
    next_handle += 1;
    
    handle_sizes[h] = size;
    handle_ptrs[h] = @alignCast(ptr.ptr);
    _ = @atomicRmw(u32, &active_vmos, .Add, 1, .monotonic);
    
    return h;
}

export fn zx_handle_close_native(handle: u32) void {
    if (handle == 0 or handle >= MAX_VMOS) return;
    
    const size = handle_sizes[handle];
    if (size > 0) {
        const ptr = handle_ptrs[handle];
        posix.munmap(ptr[0..size]);
        handle_sizes[handle] = 0; // Erase registry slot
        _ = @atomicRmw(u32, &active_vmos, .Sub, 1, .monotonic);
    }
}

export fn zx_vmo_write_native(handle: u32, offset: usize, buffer: [*]const u8, size: usize) i32 {
    if (handle == 0 or handle >= MAX_VMOS or handle_sizes[handle] == 0) return -1;
    if (offset + size > handle_sizes[handle]) return -1;
    const ptr = handle_ptrs[handle];
    @memcpy(ptr[offset..offset+size], buffer[0..size]);
    return 0;
}

export fn zx_vmo_read_native(handle: u32, offset: usize, buffer: [*]u8, size: usize) i32 {
    if (handle == 0 or handle >= MAX_VMOS or handle_sizes[handle] == 0) return -1;
    if (offset + size > handle_sizes[handle]) return -1;
    const ptr = handle_ptrs[handle];
    @memcpy(buffer[0..size], ptr[offset..offset+size]);
    return 0;
}

export fn muscle_get_active_vmos() u32 { return @atomicLoad(u32, &active_vmos, .monotonic); }
export fn muscle_get_next_handle() u32 { return next_handle; }

// --- FFI STUBS (Guarantees the Rust linker succeeds) ---
export fn zx_vmo_clone_native(parent_handle: u32, size: usize) u32 { _ = parent_handle; _ = size; return 0; }
export fn set_substrate_load(load: f32) void { _ = load; }
export fn set_stress_mode(enabled: bool) void { _ = enabled; }
export fn extract_thermal_pulse_raw() i32 { return 45000; }
export fn zx_vmo_get_info(handle: u32, out_info: *VmoInfo) i32 { _ = handle; _ = out_info; return -1; }
export fn zx_vmo_get_all_handles(out_handles: [*]u32, max_count: usize) usize { _ = out_handles; _ = max_count; return 0; }
export fn zx_vmo_calculate_entropy(handle: u32) f32 { _ = handle; return 0.0; }
