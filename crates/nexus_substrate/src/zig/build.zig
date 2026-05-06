//! NEXUS SINGULARITY — Zig Build Script v3.5 (Zig 0.15.2)
//! root_source_file points directly at muscle.zig — no lib.zig wrapper.
const std = @import("std");

pub fn build(b: *std.Build) void {
    const target   = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const muscle_mod = b.createModule(.{
        .root_source_file = b.path("../kernel/muscle.zig"),
        .target           = target,
        .optimize         = optimize,
    });

    const lib = b.addLibrary(.{
        .name        = "muscle",
        .linkage     = .static,
        .root_module = muscle_mod,
    });

    lib.bundle_compiler_rt = true;
    b.installArtifact(lib);
}
