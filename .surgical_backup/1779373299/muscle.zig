//! NEXUS SINGULARITY — ZIG MUSCLE v4.4.2 [KERNEL_SUTURE_APPLIED]
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
var alloc_mutex = Thread.Mutex{};
var free_list: [MAX_SLOTS]u16 = undefined;
var free_count: usize = 0;
var high_water: u16 = 0;
var generation_table: [MAX_SLOTS]u20 = [_]u20{1} ** MAX_SLOTS;

const VmoMetadata = struct {
    ptr: [*]align(PAGE_SIZE) u8,
    size: usize,
    magic: u32,
    generation: u32,
    refcount: u32,
    is_cow_child: bool,
    parent_handle: u32,
};

var handle_table: [MAX_SLOTS]?VmoMetadata = [_]?VmoMetadata{null} ** MAX_SLOTS;

inline fn shard_for(h: u32) usize { return @intCast(h & SHARD_MASK); }
inline fn decode_slot(h: u32) usize { return @intCast(h & SLOT_MASK); }
inline fn decode_generation(h: u32) u32 { return h >> GENERATION_SHIFT; }

fn allocate_handle() ?u32 {
    alloc_mutex.lock();
    defer alloc_mutex.unlock();
    var slot: u16 = 0;
    if (free_count > 0) {
        free_count -= 1;
        slot = free_list[free_count];
    } else if (high_water < MAX_SLOTS) {
        slot = high_water;
        high_water += 1;
    } else { return null; }
    const gen = generation_table[slot];
    return (@as(u32, gen) << GENERATION_SHIFT) | @as(u32, slot);
}

fn release_handle(h: u32) void {
    const slot = decode_slot(h);
    alloc_mutex.lock();
    defer alloc_mutex.unlock();
    generation_table[slot] +%= 1;
    if (generation_table[slot] == 0) generation_table[slot] = 1;
    free_list[free_count] = @intCast(slot);
    free_count += 1;
}

fn lookup_valid(h: u32) ?*VmoMetadata {
    const slot = decode_slot(h);
    const gen  = decode_generation(h);
    const meta = &(handle_table[slot] orelse return null);
    if (meta.magic != SENTINEL_MAGIC) return null;
    if (meta.generation != gen) return null;
    return meta;
}

fn allocate_vmo(size: usize) !struct { ptr: [*]align(PAGE_SIZE) u8, size: usize } {
    const aligned_size = std.mem.alignForward(usize, size, PAGE_SIZE);
    const ptr = try posix.mmap(null, aligned_size, posix.PROT.READ | posix.PROT.WRITE, .{ .TYPE = .PRIVATE, .ANONYMOUS = true }, -1, 0);
    return .{ .ptr = @alignCast(ptr.ptr), .size = aligned_size };
}

export fn zx_vmo_create_native(size: usize) u32 {
    const mapping = allocate_vmo(size) catch return 0;
    const h = allocate_handle() orelse { posix.munmap(mapping.ptr[0..mapping.size]); return 0; };
    const shard = shard_for(h);
    const slot  = decode_slot(h);
    const gen   = decode_generation(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    handle_table[slot] = VmoMetadata{ .ptr = mapping.ptr, .size = mapping.size, .magic = SENTINEL_MAGIC, .generation = gen, .refcount = 1, .is_cow_child = false, .parent_handle = 0 };
    _ = @atomicRmw(u32, &active_vmos, .Add, 1, .monotonic);
    return h;
}

export fn zx_handle_close_native(h: u32) void {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    const meta = lookup_valid(h) orelse { shard_mutexes[shard].unlock(); return; };
    const old_ref = @atomicRmw(u32, &meta.refcount, .Sub, 1, .monotonic);
    if (old_ref == 1) {
        posix.munmap(meta.ptr[0..meta.size]);
        handle_table[decode_slot(h)] = null;
        shard_mutexes[shard].unlock();
        release_handle(h);
        _ = @atomicRmw(u32, &active_vmos, .Sub, 1, .monotonic);
    } else { shard_mutexes[shard].unlock(); }
}

export fn muscle_get_active_vmos() u32 { return @atomicLoad(u32, &active_vmos, .monotonic); }

export fn zx_vmo_get_info(h: u32, out_info: *VmoInfo) i32 {
    const shard = shard_for(h);
    shard_mutexes[shard].lock();
    defer shard_mutexes[shard].unlock();
    const meta = lookup_valid(h) orelse return -1;
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
                out_handles[count] = slot | (@as(u32, meta.generation) << GENERATION_SHIFT);
                count += 1;
            }
        }
    }
    return count;
}
