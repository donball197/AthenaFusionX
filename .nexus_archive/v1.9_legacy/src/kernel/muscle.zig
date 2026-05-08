//! NEXUS SINGULARITY — ZIG MUSCLE v3.2 (2026)
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

var shard_mutexes: [SHARD_COUNT]Thread.Mutex = [_]Thread.Mutex{.{}} ** SHARD_COUNT;
var active_vmos: u32 = 0;
var next_handle: u32 = 0xc8;

const VmoMetadata = struct {
    ptr:           [*]align(PAGE_SIZE) u8,
    size:          usize,
    magic:         u32,
    generation:    u32,
    refcount:      u32,
    is_cow_child:  bool,
    parent_handle: u32,
};

const VmoInfo = extern struct {
    handle:          u32,
    size:            usize,
    committed_pages: usize,
    is_cow:          u8,
    parent_handle:   u32,
};

var handle_table: [MAX_SLOTS]?VmoMetadata = [_]?VmoMetadata{null} ** MAX_SLOTS;
var current_load: f32 = 0.0;
var stress_mode: bool = false;

inline fn shard_for(h: u32) usize { return @intCast(h & SHARD_MASK); }
inline fn decode_slot(h: u32) usize { return @intCast(h & SLOT_MASK); }
inline fn decode_generation(h: u32) u32 { return h >> GENERATION_SHIFT; }

fn lookup_valid(h: u32) ?*VmoMetadata {
    const slot = decode_slot(h);
    const gen  = decode_generation(h);
    const meta = &(handle_table[slot] orelse return null);
    if (meta.magic != SENTINEL_MAGIC) return null;
    if (meta.generation != gen)       return null;
    return meta;
}

fn allocate_vmo(size: usize) !struct { ptr: [*]align(PAGE_SIZE) u8, size: usize } {
    const aligned_size = std.mem.alignForward(usize, size, PAGE_SIZE);
    const ptr = try posix.mmap(null, aligned_size, posix.PROT.READ | posix.PROT.WRITE, .{ .TYPE = .PRIVATE, .ANONYMOUS = true }, -1, 0);
    return .{ .ptr = @alignCast(ptr.ptr), .size = aligned_size };
}

export fn zx_vmo_create_native(size: usize) u32 {
    const mapping = allocate_vmo(size) catch return 0;
    const h     = @atomicRmw(u32, &next_handle, .Add, 1, .monotonic);
    const shard = shard_for(h);
    const slot  = decode_slot(h);
    const gen   = decode_generation(h);

    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();

    handle_table[slot] = VmoMetadata{
        .ptr           = mapping.ptr,
        .size          = mapping.size,
        .magic         = SENTINEL_MAGIC,
        .generation    = gen,
        .refcount      = 1,
        .is_cow_child  = false,
        .parent_handle = 0,
    };
    _ = @atomicRmw(u32, &active_vmos, .Add, 1, .monotonic);
    return h;
}

export fn zx_vmo_clone_native(parent_handle: u32, _: usize) u32 {
    const p_shard = shard_for(parent_handle);
    var parent_size: usize = 0;
    var parent_ptr:  [*]align(PAGE_SIZE) u8 = undefined;

    {
        shard_mutexes[p_shard].lock();
        defer shard_mutexes[p_shard].unlock();
        const meta = lookup_valid(parent_handle) orelse return 0;
        parent_size = meta.size;
        parent_ptr  = meta.ptr;
        _ = @atomicRmw(u32, &meta.refcount, .Add, 1, .monotonic);
    }

    const mapping = allocate_vmo(parent_size) catch return 0;
    @memcpy(mapping.ptr[0..parent_size], parent_ptr[0..parent_size]);

    const h     = @atomicRmw(u32, &next_handle, .Add, 1, .monotonic);
    const shard = shard_for(h);
    const slot  = decode_slot(h);
    const gen   = decode_generation(h);

    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();

    handle_table[slot] = VmoMetadata{
        .ptr           = mapping.ptr,
        .size          = mapping.size,
        .magic         = SENTINEL_MAGIC,
        .generation    = gen,
        .refcount      = 1,
        .is_cow_child  = true,
        .parent_handle = parent_handle,
    };

    _ = @atomicRmw(u32, &active_vmos, .Add, 1, .monotonic);
    return h;
}

export fn zx_handle_close_native(h: u32) void {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();

    const meta = lookup_valid(h) orelse return;

    const old_ref = @atomicRmw(u32, &meta.refcount, .Sub, 1, .monotonic);
    if (old_ref == 1) {
        posix.munmap(meta.ptr[0..meta.size]);
        handle_table[decode_slot(h)] = null;
        _ = @atomicRmw(u32, &active_vmos, .Sub, 1, .monotonic);
    }
}

export fn zx_vmo_write_native(h: u32, offset: usize, buffer: [*]const u8, size: usize) i32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return -1;
    if (offset + size > meta.size) return -1;
    @memcpy(meta.ptr[offset .. offset + size], buffer[0..size]);
    return 0;
}

export fn zx_vmo_read_native(h: u32, offset: usize, buffer: [*]u8, size: usize) i32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return -1;
    if (offset + size > meta.size) return -1;
    @memcpy(buffer[0..size], meta.ptr[offset .. offset + size]);
    return 0;
}

export fn zx_vmo_get_info(h: u32, out_info: *VmoInfo) i32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return -1;
    out_info.* = .{
        .handle          = h,
        .size            = meta.size,
        .committed_pages = meta.size / PAGE_SIZE,
        .is_cow          = if (meta.is_cow_child) 1 else 0,
        .parent_handle   = meta.parent_handle,
    };
    return 0;
}

export fn zx_vmo_calculate_entropy(h: u32) f32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return 0.0;
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
    for (&shard_mutexes) |*m| m.lock();
    defer for (&shard_mutexes) |*m| m.unlock();
    var count: usize = 0;
    for (handle_table, 0..) |maybe_meta, i| {
        if (maybe_meta) |meta| {
            if (meta.magic == SENTINEL_MAGIC and count < max_count) {
                const slot: u32 = @intCast(i);
                out_handles[count] = slot | (meta.generation << GENERATION_SHIFT);
                count += 1;
            }
        }
    }
    return count;
}

export fn muscle_get_active_vmos() u32 { return @atomicLoad(u32, &active_vmos, .monotonic); }
export fn muscle_get_next_handle() u32 { return @atomicLoad(u32, &next_handle, .monotonic); }
export fn set_substrate_load(load: f32) void {
    shard_mutexes[0].lock();
    defer shard_mutexes[0].unlock();
    current_load = @max(0.0, @min(1.0, load));
}
export fn set_stress_mode(enabled: bool) void {
    shard_mutexes[0].lock();
    defer shard_mutexes[0].unlock();
    stress_mode = enabled;
}
export fn extract_thermal_pulse_raw() i32 {
    shard_mutexes[0].lock();
    defer shard_mutexes[0].unlock();
    const multiplier: f32 = if (stress_mode) 2.5 else 1.0;
    const temp = (current_load * 40.0 * multiplier) + 30.0;
    return @intFromFloat(temp * 1000.0);
}
