// Project Nexus Singularity v0.2.2
// "Real Robotic Kernel" - Zircon-to-Zig Bridge
// Optimized for ARM64 (Motorola/Chromebook)

const zx = @import("zircon_types.zig");

pub export fn nexus_vmo_clone(handle: u32, options: u32, offset: u64, size: u64, out: *u32) i32 {
    return @extern(i32, "zx_vmo_clone", .{handle, options, offset, size, out});
}

pub export fn nexus_vmar_map(
    handle: u32,
    options: u32,
    vmar_offset: u64,
    vmo: u32,
    vmo_offset: u64,
    len: u64,
    mapped_addr: *usize,
) i32 {
    return @extern(i32, "zx_vmar_map", .{handle, options, vmar_offset, vmo, vmo_offset, len, mapped_addr});
}

pub export fn nexus_vmar_protect(handle: u32, options: u32, addr: usize, len: u64) i32 {
    return @extern(i32, "zx_vmar_protect", .{handle, options, addr, len});
}

pub export fn nexus_vmar_destroy(handle: u32) i32 {
    return @extern(i32, "zx_vmar_destroy", .{handle});
}
