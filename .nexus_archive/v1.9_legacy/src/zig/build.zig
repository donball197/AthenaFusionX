const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{
        .default_target = .{ .cpu_arch = .wasm32, .os_tag = .freestanding },
    });
    const optimize = b.standardOptimizeOption(.{ .preferred_optimize_mode = .ReleaseSmall });

    // For freestanding WASM in Wasmtime, we build an Executable with no entry point
    const lib = b.addExecutable(.{
        .name = "nexus_zig_cell",
        .root_module = b.createModule(.{
            .root_source_file = b.path("lib.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });

    lib.rdynamic = true;
    lib.entry = .disabled;
    b.installArtifact(lib);
}
