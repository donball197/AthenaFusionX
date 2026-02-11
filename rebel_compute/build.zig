const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // 1. Define the Module (The logical heart of the kernel)
    const rebel_mod = b.createModule(.{
        .root_source_file = b.path("src/kernels.zig"),
        .target = target,
        .optimize = optimize,
    });

    // 2. Wrap it in a Library with static linkage for the Rust weld
    const lib = b.addLibrary(.{
        .name = "rebel_kernels",
        .root_module = rebel_mod,
        .linkage = .static,
    });

    // 3. Deploy the artifact so Rust can find it in zig-out/lib
    b.installArtifact(lib);
}
