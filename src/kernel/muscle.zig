const std = @import("std");
const posix = std.posix;

// ─── VMO ENGINE STATE ─────────────────────────────────────────────────────────
var active_vmos: u32 = 0;
var next_handle: u32 = 0xc8;
const GENERATION_SHIFT: u5 = 12;
const MAX_SLOTS: usize = 4096;
const SLOT_MASK: u32 = 4095;
const PAGE_SIZE: usize = 4096;
const SENTINEL_MAGIC: u32 = 0xDEADC0DE;

// ─── SYNTHETIC REFLEX STATE ──────────────────────────────────────────────────
var current_load: f32 = 0.0;

const VmoMetadata = struct {
    ptr: [*]align(PAGE_SIZE) u8,
    size: usize,
    magic: u32,
    generation: u32,
    refcount: u32,
    is_cow_child: bool = false,
    parent_slot: u32 = 0,
    parent_handle: u32 = 0,
    dirty_pages: std.DynamicBitSetUnmanaged,
};

var handle_table: [MAX_SLOTS]?VmoMetadata = [_]?VmoMetadata{null} ** MAX_SLOTS;
var free_slots: [MAX_SLOTS]u32 = undefined;
var free_count: usize = 0;

fn allocate_handle_slot() ?u32 {
    if (free_count > 0) {
        free_count -= 1;
        return free_slots[free_count];
    }
    if (next_handle >= 0xFFFFFFFF) return null;
    const slot = next_handle & SLOT_MASK;
    next_handle += 1;
    return slot;
}

fn release_slot(slot: u32) void {
    if (free_count < MAX_SLOTS) {
        free_slots[free_count] = slot;
        free_count += 1;
    }
}

// ─── SUBSTRATE LOAD PUMP ─────────────────────────────────────────────────────

export fn set_substrate_load(load: f32) void {
    current_load = load;
}

export fn extract_thermal_pulse_raw() i32 {
    // Synthetic: 30°C idle → 70°C full load
    const temp = (current_load * 40.0) + 30.0;
    return @intFromFloat(temp * 1000.0);
}

// ─── VMO CREATE ──────────────────────────────────────────────────────────────

export fn zx_vmo_create_native(size: usize) u32 {
    const aligned_size = (size + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
    const ptr = posix.mmap(
        null, aligned_size,
        posix.PROT.READ | posix.PROT.WRITE,
        .{ .TYPE = .PRIVATE, .ANONYMOUS = true },
        -1, 0,
    ) catch return 0;

    if (allocate_handle_slot()) |slot| {
        const handle = (next_handle << GENERATION_SHIFT) | slot;
        const dirty = std.DynamicBitSetUnmanaged.initEmpty(
            std.heap.page_allocator,
            aligned_size / PAGE_SIZE,
        ) catch {
            posix.munmap(ptr[0..aligned_size]);
            return 0;
        };

        handle_table[slot] = VmoMetadata{
            .ptr        = @alignCast(ptr.ptr),
            .size       = aligned_size,
            .magic      = SENTINEL_MAGIC,
            .generation = next_handle >> GENERATION_SHIFT,
            .refcount   = 1,
            .is_cow_child = false,
            .dirty_pages  = dirty,
        };
        active_vmos += 1;
        return handle;
    }
    posix.munmap(ptr[0..aligned_size]);
    return 0;
}

// ─── VMO CLONE (COW) ─────────────────────────────────────────────────────────

export fn zx_vmo_clone_native(parent_handle: u32, requested_size: usize) u32 {
    const parent_slot = parent_handle & SLOT_MASK;
    if (handle_table[parent_slot]) |*parent_meta| {
        if (parent_meta.magic != SENTINEL_MAGIC) return 0;
        parent_meta.refcount += 1;

        const size = if (requested_size == 0) parent_meta.size else requested_size;
        const aligned_size = (size + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);

        if (allocate_handle_slot()) |slot| {
            const new_handle = (next_handle << GENERATION_SHIFT) | slot;
            const dirty = std.DynamicBitSetUnmanaged.initEmpty(
                std.heap.page_allocator,
                aligned_size / PAGE_SIZE,
            ) catch {
                parent_meta.refcount -= 1;
                return 0;
            };

            handle_table[slot] = VmoMetadata{
                .ptr          = parent_meta.ptr, // ← shares parent pointer (COW)
                .size         = aligned_size,
                .magic        = SENTINEL_MAGIC,
                .generation   = next_handle >> GENERATION_SHIFT,
                .refcount     = 1,
                .is_cow_child = true,
                .parent_slot  = parent_slot,
                .parent_handle = parent_handle,
                .dirty_pages  = dirty,
            };
            active_vmos += 1;
            std.debug.print(
                "\x1b[36m[VMO_COW_CLONE] Parent 0x{x} -> Child 0x{x}\x1b[0m\n",
                .{ parent_handle, new_handle },
            );
            return new_handle;
        } else {
            parent_meta.refcount -= 1;
        }
    }
    return 0;
}

// ─── VMO WRITE ───────────────────────────────────────────────────────────────
// ⚡ FIX: On first write to a COW child, allocate independent memory,
//         copy parent content, then zero-fill from write_end to page boundary.
//         This eliminates the "CHILD_DATAATA" bleed-through artifact.

export fn zx_vmo_write_native(handle: u32, offset: usize, buffer: [*]const u8, size: usize) i32 {
    const slot = handle & SLOT_MASK;
    if (handle_table[slot]) |*meta| {
        if (meta.magic != SENTINEL_MAGIC) return -1;
        if (offset + size > meta.size) return -1;

        if (meta.is_cow_child) {
            // Eager fork: allocate private memory for child on first write
            const new_ptr = posix.mmap(
                null, meta.size,
                posix.PROT.READ | posix.PROT.WRITE,
                .{ .TYPE = .PRIVATE, .ANONYMOUS = true },
                -1, 0,
            ) catch return -1;

            // Copy parent content into new private page
            @memcpy(new_ptr[0..meta.size], meta.ptr[0..meta.size]);

            // ⚡ KEY FIX: Zero-fill from (offset + size) to end of allocation.
            // This prevents stale parent bytes bleeding past the write boundary.
            if (offset + size < meta.size) {
                @memset(new_ptr[offset + size .. meta.size], 0);
            }

            meta.ptr = @alignCast(new_ptr.ptr);
            meta.is_cow_child = false; // Now an independent VMO
        }

        @memcpy(meta.ptr[offset .. offset + size], buffer[0..size]);
        return 0;
    }
    return -1;
}

// ─── VMO READ ────────────────────────────────────────────────────────────────

export fn zx_vmo_read_native(handle: u32, offset: usize, buffer: [*]u8, size: usize) i32 {
    const slot = handle & SLOT_MASK;
    if (handle_table[slot]) |meta| {
        if (meta.magic != SENTINEL_MAGIC) return -1;
        if (offset + size > meta.size) return -1;
        @memcpy(buffer[0..size], meta.ptr[offset .. offset + size]);
        return 0;
    }
    return -1;
}

// ─── HANDLE CLOSE ────────────────────────────────────────────────────────────

export fn zx_handle_close_native(handle: u32) void {
    const slot = handle & SLOT_MASK;
    if (handle_table[slot]) |*meta| {
        if (meta.magic != SENTINEL_MAGIC) return;
        meta.refcount -= 1;
        if (meta.refcount == 0) {
            meta.dirty_pages.deinit(std.heap.page_allocator);
            posix.munmap(meta.ptr[0..meta.size]);
            handle_table[slot] = null;
            release_slot(slot);
            active_vmos -= 1;
        }
    }
}

// ─── TELEMETRY ───────────────────────────────────────────────────────────────

export fn muscle_get_active_vmos() u32 { return active_vmos; }
export fn muscle_get_next_handle() u32 { return next_handle; }

const VmoInfo = extern struct {
    handle: u32,
    size: usize,
    committed_pages: usize,
    is_cow: bool,
    parent_handle: u32,
};

export fn zx_vmo_get_info(handle: u32, out_info: *VmoInfo) i32 {
    const slot = handle & SLOT_MASK;
    if (handle_table[slot]) |meta| {
        if (meta.magic != SENTINEL_MAGIC) return -1;
        out_info.* = VmoInfo{
            .handle          = handle,
            .size            = meta.size,
            .committed_pages = if (meta.is_cow_child)
                meta.dirty_pages.count()
            else
                (meta.size / PAGE_SIZE),
            .is_cow          = meta.is_cow_child,
            .parent_handle   = meta.parent_handle,
        };
        return 0;
    }
    return -1;
}

export fn zx_vmo_get_all_handles(out_handles: [*]u32, max_count: usize) usize {
    var count: usize = 0;
    for (handle_table, 0..) |meta_opt, slot| {
        if (meta_opt) |meta| {
            if (count < max_count) {
                out_handles[count] =
                    (meta.generation << GENERATION_SHIFT) | @as(u32, @intCast(slot));
                count += 1;
            }
        }
    }
    return count;
}
