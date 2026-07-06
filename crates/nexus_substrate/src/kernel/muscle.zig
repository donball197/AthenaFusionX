//! NEXUS SINGULARITY — ZIG MUSCLE v4.4.2 [STABILIZED]
const std = @import("std");
const posix = std.posix;
const Thread = std.Thread;

const PAGE_SIZE: usize = 4096;
const MAX_SLOTS: usize = 4096;
const SLOT_MASK: u32 = MAX_SLOTS - 1;
const SENTINEL_MAGIC: u32 = 0xDEADC0DE;
const GENERATION_SHIFT: u5 = 12;
const SHARD_COUNT: usize = 16;
const SHARD_MASK: u32 = SHARD_COUNT - 1;

pub const VmoInfo = extern struct {
    handle: u32,
    size: usize,
    committed_pages: usize,
    is_cow: u8,
    parent_handle: u32,
};

pub const VmoMetadata = struct {
    ptr: [*]align(PAGE_SIZE) u8,
    size: usize,
    magic: u32,
    generation: u32,
    refcount: u32,
    is_cow_child: bool,
    parent_handle: u32,
};

var shard_mutexes: [SHARD_COUNT]Thread.Mutex = [_]Thread.Mutex{.{}} ** SHARD_COUNT;
var active_vmos: u32 = 0;
var next_handle: u32 = 0xc8;
var handle_table: [MAX_SLOTS]?VmoMetadata = [_]?VmoMetadata{null} ** MAX_SLOTS;
var generation_table: [MAX_SLOTS]u20 = [_]u20{1} ** MAX_SLOTS;
var current_load: f32 = 0.0;
var stress_mode: bool = false;

inline fn shard_for(h: u32) usize { return @intCast(h & SHARD_MASK); }
inline fn decode_slot(h: u32) usize { return @intCast(h & SLOT_MASK); }
inline fn decode_generation(h: u32) u32 { return h >> GENERATION_SHIFT; }

export fn zx_vmo_create_native(size: usize) u32 {
    if (next_handle >= MAX_SLOTS) return 0;
    const flags = posix.MAP{ .TYPE = .PRIVATE, .ANONYMOUS = true };
    const ptr = posix.mmap(null, size, posix.PROT.READ | posix.PROT.WRITE, flags, -1, 0) catch return 0;

    const h = next_handle;
    next_handle += 1;

    handle_table[decode_slot(h)] = VmoMetadata{
        .ptr = @alignCast(ptr.ptr),
        .size = size,
        .magic = SENTINEL_MAGIC,
        .generation = @intCast(decode_generation(h)),
        .refcount = 1,
        .is_cow_child = false,
        .parent_handle = 0,
    };
    _ = @atomicRmw(u32, &active_vmos, .Add, 1, .monotonic);
    return h;
}

export fn zx_handle_close_native(handle: u32) void {
    const slot = decode_slot(handle);
    if (handle_table[slot]) |meta| {
        if (meta.magic == SENTINEL_MAGIC) {
            posix.munmap(meta.ptr[0..meta.size]);
            handle_table[slot] = null;
            _ = @atomicRmw(u32, &active_vmos, .Sub, 1, .monotonic);
        }
    }
}

export fn zx_vmo_write_native(handle: u32, offset: usize, buffer: [*]const u8, size: usize) i32 {
    const slot = decode_slot(handle);
    const meta = handle_table[slot] orelse return -1;
    if (meta.magic != SENTINEL_MAGIC or offset + size > meta.size) return -1;
    @memcpy(meta.ptr[offset..offset+size], buffer[0..size]);
    return 0;
}

export fn zx_vmo_read_native(handle: u32, offset: usize, buffer: [*]u8, size: usize) i32 {
    const slot = decode_slot(handle);
    const meta = handle_table[slot] orelse return -1;
    if (meta.magic != SENTINEL_MAGIC or offset + size > meta.size) return -1;
    @memcpy(buffer[0..size], meta.ptr[offset..offset+size]);
    return 0;
}

export fn zx_vmo_get_info(h: u32, out_info: *VmoInfo) i32 {
    const slot = decode_slot(h);
    const gen = decode_generation(h);
    const meta = handle_table[slot] orelse return -1;
    if (meta.magic != SENTINEL_MAGIC or meta.generation != gen) return -1;

    out_info.* = .{
        .handle = h,
        .size = @intCast(meta.size),
        .committed_pages = @intCast(meta.size / PAGE_SIZE),
        .is_cow = if (meta.is_cow_child) 1 else 0,
        .parent_handle = meta.parent_handle,
    };
    return 0;
}

export fn zx_vmo_calculate_entropy(h: u32) f32 {
    const slot = decode_slot(h);
    const gen = decode_generation(h);
    const meta = handle_table[slot] orelse return 0.0;
    if (meta.magic != SENTINEL_MAGIC or meta.generation != gen) return 0.0;

    var counts = [_]u32{0} ** 256;
    for (meta.ptr[0..meta.size]) |byte| counts[byte] += 1;
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

export fn zx_vmo_get_all_handles(out_handles: [*]u32, max_count: usize) usize {
    var count: usize = 0;
    for (handle_table, 0..) |maybe_meta, i| {
        if (maybe_meta) |meta| {
            if (meta.magic == SENTINEL_MAGIC and count < max_count) {
                out_handles[count] = @as(u32, @intCast(i)) | (@as(u32, meta.generation) << GENERATION_SHIFT);
                count += 1;
            }
        }
    }
    return count;
}

export fn muscle_get_active_vmos() u32 { return @atomicLoad(u32, &active_vmos, .monotonic); }
export fn muscle_get_next_handle() u32 { return next_handle; }

// FFI Stubs to satisfy the linker
export fn zx_vmo_clone_native(parent_handle: u32, size: usize) u32 { _ = parent_handle; _ = size; return 0; }
export fn set_substrate_load(load: f32) void { current_load = load; }
export fn set_stress_mode(enabled: bool) void { stress_mode = enabled; }
export fn extract_thermal_pulse_raw() i32 { return 45000; }
