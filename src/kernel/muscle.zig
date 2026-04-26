//! NEXUS SINGULARITY — ZIG MUSCLE v2.0 (2026)
//! Hardened VMO substrate with reduced contention and better safety.

const std = @import("std");
const posix = std.posix;
const Thread = std.Thread;

const PAGE_SIZE: usize = 4096;
const MAX_SLOTS: usize = 4096;
const SLOT_MASK: u32 = MAX_SLOTS - 1;
const SENTINEL_MAGIC: u32 = 0xDEADC0DE;
const GENERATION_SHIFT: u5 = 12;

var active_vmos: u32 = 0;
var next_handle: u32 = 0xc8; // start high to avoid small integer confusion

// Reduced global state — still using one mutex for simplicity.
// For high-throughput production, consider sharded locks or lock-free structures.
var global_mutex = Thread.Mutex{};

const VmoMetadata = struct {
    ptr: [*]align(PAGE_SIZE) u8,
    size: usize,
    magic: u32,
    generation: u32,
    refcount: u32,
    is_cow_child: bool,
    parent_handle: u32 = 0,
};

const VmoInfo = extern struct {
    handle: u32,
    size: usize,
    committed_pages: usize,
    is_cow: u8,
    parent_handle: u32,
};

var handle_table: [MAX_SLOTS]?VmoMetadata = [_]?VmoMetadata{null} ** MAX_SLOTS;

// Thermal / Stress state (protected by mutex)
var current_load: f32 = 0.0;
var stress_mode: bool = false;

export fn set_substrate_load(load: f32) void {
    global_mutex.lock();
    defer global_mutex.unlock();
    current_load = @max(0.0, @min(1.0, load)); // clamp
}

export fn set_stress_mode(enabled: bool) void {
    global_mutex.lock();
    defer global_mutex.unlock();
    stress_mode = enabled;
}

export fn extract_thermal_pulse_raw() i32 {
    global_mutex.lock();
    defer global_mutex.unlock();
    const multiplier: f32 = if (stress_mode) 2.5 else 1.0;
    const temp = (current_load * 40.0 * multiplier) + 30.0;
    return @intFromFloat(temp * 1000.0);
}

export fn zx_vmo_calculate_entropy(h: u32) f32 {
    global_mutex.lock();
    defer global_mutex.unlock();
    const slot = h & SLOT_MASK;
    const meta = handle_table[slot] orelse return 0.0;
    if (meta.magic != SENTINEL_MAGIC) return 0.0;

    var counts = [_]u32{0} ** 256;
    for (meta.ptr[0..meta.size]) |byte| {
        counts[byte] += 1;
    }

    var entropy: f32 = 0.0;
    const size_f: f32 = @floatFromInt(meta.size);
    for (counts) |count| {
        if (count > 0) {
            const p = @as(f32, @floatFromInt(count)) / size_f;
            entropy -= p * std.math.log2(p);
        }
    }
    return entropy / 8.0;
}

fn allocate_vmo(size: usize) !struct { ptr: [*]align(PAGE_SIZE) u8, size: usize } {
    const aligned_size = std.mem.alignForward(usize, size, PAGE_SIZE);
    const ptr = try posix.mmap(null, aligned_size,
        posix.PROT.READ | posix.PROT.WRITE,
        .{ .TYPE = .PRIVATE, .ANONYMOUS = true }, -1, 0);
    return .{ .ptr = @alignCast(ptr.ptr), .size = aligned_size };
}

export fn zx_vmo_create_native(size: usize) u32 {
    global_mutex.lock();
    defer global_mutex.unlock();

    const mapping = allocate_vmo(size) catch return 0;
    const slot = next_handle & SLOT_MASK;

    handle_table[slot] = VmoMetadata{
        .ptr = mapping.ptr,
        .size = mapping.size,
        .magic = SENTINEL_MAGIC,
        .generation = next_handle >> GENERATION_SHIFT,
        .refcount = 1,
        .is_cow_child = false,
    };

    const handle = next_handle;
    next_handle +%= 1; // wrap safely
    active_vmos += 1;
    return handle;
}

export fn zx_vmo_clone_native(parent_handle: u32, _: usize) u32 {
    global_mutex.lock();
    defer global_mutex.unlock();

    const p_slot = parent_handle & SLOT_MASK;
    const parent = handle_table[p_slot] orelse return 0;
    if (parent.magic != SENTINEL_MAGIC) return 0;

    const mapping = allocate_vmo(parent.size) catch return 0;
    @memcpy(mapping.ptr[0..parent.size], parent.ptr[0..parent.size]);

    const slot = next_handle & SLOT_MASK;
    handle_table[slot] = VmoMetadata{
        .ptr = mapping.ptr,
        .size = mapping.size,
        .magic = SENTINEL_MAGIC,
        .generation = next_handle >> GENERATION_SHIFT,
        .refcount = 1,
        .is_cow_child = true,
        .parent_handle = parent_handle,
    };

    const handle = next_handle;
    next_handle +%= 1;
    active_vmos += 1;
    return handle;
}

export fn zx_vmo_write_native(h: u32, offset: usize, buffer: [*]const u8, size: usize) i32 {
    global_mutex.lock();
    defer global_mutex.unlock();

    const slot = h & SLOT_MASK;
    const meta = &(handle_table[slot] orelse return -1);
    if (meta.magic != SENTINEL_MAGIC) return -1;
    if (offset + size > meta.size) return -1;

    @memcpy(meta.ptr[offset .. offset + size], buffer[0..size]);
    return 0;
}

export fn zx_vmo_read_native(h: u32, offset: usize, buffer: [*]u8, size: usize) i32 {
    global_mutex.lock();
    defer global_mutex.unlock();

    const slot = h & SLOT_MASK;
    const meta = handle_table[slot] orelse return -1;
    if (meta.magic != SENTINEL_MAGIC) return -1;
    if (offset + size > meta.size) return -1;

    @memcpy(buffer[0..size], meta.ptr[offset .. offset + size]);
    return 0;
}

export fn zx_handle_close_native(h: u32) void {
    global_mutex.lock();
    defer global_mutex.unlock();

    const slot = h & SLOT_MASK;
    const meta = &(handle_table[slot] orelse return);
    if (meta.magic != SENTINEL_MAGIC) return;

    meta.refcount -%= 1;
    if (meta.refcount == 0) {
        // ⚡ SUTURE: munmap returns void in Zig 0.13.0+
        posix.munmap(meta.ptr[0..meta.size]);
        handle_table[slot] = null;
        active_vmos -%= 1;
    }
}

export fn muscle_get_active_vmos() u32 { return active_vmos; }
export fn muscle_get_next_handle() u32 { return next_handle; }

export fn zx_vmo_get_info(h: u32, out_info: *VmoInfo) i32 {
    global_mutex.lock();
    defer global_mutex.unlock();

    const slot = h & SLOT_MASK;
    const meta = handle_table[slot] orelse return -1;
    if (meta.magic != SENTINEL_MAGIC) return -1;

    out_info.* = .{
        .handle = h,
        .size = meta.size,
        .committed_pages = meta.size / PAGE_SIZE,
        .is_cow = if (meta.is_cow_child) 1 else 0,
        .parent_handle = meta.parent_handle,
    };
    return 0;
}

export fn zx_vmo_get_all_handles(out_handles: [*]u32, max_count: usize) usize {
    global_mutex.lock();
    defer global_mutex.unlock();

    var count: usize = 0;
    for (handle_table, 0..) |maybe_meta, i| {
        if (maybe_meta) |meta| {
            if (meta.magic == SENTINEL_MAGIC and count < max_count) {
                out_handles[count] = @intCast(i);
                count += 1;
            }
        }
    }
    return count;
}
