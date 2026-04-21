const std = @import("std");
const posix = std.posix;

var active_vmos: u32 = 0;
var next_handle: u32 = 0xc8;
const MAX_SLOTS: usize = 4096;
const SLOT_MASK: u32 = 4095;
const PAGE_SIZE: usize = 4096;
const SENTINEL_MAGIC: u32 = 0xDEADC0DE;
const GENERATION_SHIFT: u5 = 12;

var current_load: f32 = 0.0;

const VmoMetadata = struct {
    ptr: [*]align(PAGE_SIZE) u8,
    size: usize,
    magic: u32,
    generation: u32,
    refcount: u32,
    is_cow_child: bool,
    parent_handle: u32 = 0,
};

var handle_table: [MAX_SLOTS]?VmoMetadata = [_]?VmoMetadata{null} ** MAX_SLOTS;

export fn set_substrate_load(load: f32) void { current_load = load; }
export fn extract_thermal_pulse_raw() i32 {
    const temp = (current_load * 40.0) + 30.0;
    return @intFromFloat(temp * 1000.0);
}

export fn zx_vmo_create_native(size: usize) u32 {
    const aligned_size = (size + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
    const ptr = posix.mmap(null, aligned_size, posix.PROT.READ | posix.PROT.WRITE, .{ .TYPE = .PRIVATE, .ANONYMOUS = true }, -1, 0) catch return 0;
    const slot = next_handle & SLOT_MASK;
    handle_table[slot] = VmoMetadata{
        .ptr = @alignCast(ptr.ptr), .size = aligned_size, .magic = SENTINEL_MAGIC,
        .generation = next_handle >> GENERATION_SHIFT, .refcount = 1, .is_cow_child = false,
    };
    const handle = next_handle;
    next_handle += 1;
    active_vmos += 1;
    return handle;
}

export fn zx_vmo_clone_native(parent_handle: u32, _: usize) u32 {
    const p_slot = parent_handle & SLOT_MASK;
    if (handle_table[p_slot]) |*p| {
        const slot = next_handle & SLOT_MASK;
        handle_table[slot] = VmoMetadata{
            .ptr = p.ptr, .size = p.size, .magic = SENTINEL_MAGIC,
            .generation = next_handle >> GENERATION_SHIFT, .refcount = 1,
            .is_cow_child = true, .parent_handle = parent_handle,
        };
        const handle = next_handle;
        next_handle += 1;
        active_vmos += 1;
        return handle;
    }
    return 0;
}

export fn zx_vmo_write_native(h: u32, o: usize, b: [*]const u8, s: usize) i32 {
    const slot = h & SLOT_MASK;
    if (handle_table[slot]) |*meta| {
        if (o + s > meta.size) return -1;
        @memcpy(meta.ptr[o .. o + s], b[0..s]);
        return 0;
    }
    return -1;
}

export fn zx_vmo_read_native(h: u32, o: usize, b: [*]u8, s: usize) i32 {
    const slot = h & SLOT_MASK;
    if (handle_table[slot]) |meta| {
        if (o + s > meta.size) return -1;
        @memcpy(b[0..s], meta.ptr[o .. o + s]);
        return 0;
    }
    return -1;
}

export fn zx_handle_close_native(h: u32) void {
    const slot = h & SLOT_MASK;
    if (handle_table[slot]) |*meta| {
        meta.refcount -= 1;
        if (meta.refcount == 0) {
            posix.munmap(meta.ptr[0..meta.size]);
            handle_table[slot] = null;
            active_vmos -= 1;
        }
    }
}

export fn muscle_get_active_vmos() u32 { return active_vmos; }
export fn muscle_get_next_handle() u32 { return next_handle; }

const VmoInfo = extern struct {
    handle: u32, size: usize, committed_pages: usize, is_cow: u8, parent_handle: u32,
};

export fn zx_vmo_get_info(h: u32, out_info: *VmoInfo) i32 {
    const slot = h & SLOT_MASK;
    if (handle_table[slot]) |meta| {
        out_info.* = VmoInfo{
            .handle = h, .size = meta.size, .committed_pages = meta.size / PAGE_SIZE,
            .is_cow = if (meta.is_cow_child) 1 else 0, .parent_handle = meta.parent_handle,
        };
        return 0;
    }
    return -1;
}

export fn zx_vmo_get_all_handles(out_handles: [*]u32, max_count: usize) usize {
    var count: usize = 0;
    for (handle_table, 0..) |m, i| {
        if (m != null and count < max_count) {
            out_handles[count] = @intCast(i);
            count += 1;
        }
    }
    return count;
}
