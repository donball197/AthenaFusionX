const std = @import("std");
const posix = std.posix;

// EXPLICIT EXPORT FOR LINKER VISIBILITY
export fn resonator_ignite(path_ptr: [*]const u8, path_len: usize) i32 {
    const path = path_ptr[0..path_len];
    const file = std.fs.cwd().openFile(path, .{}) catch return -1;
    defer file.close();
    
    const stat = file.stat() catch return -2;

    // MODERN ZIG TYPE-SAFE FLAGS
    const prot = posix.PROT.READ;
    
    // On Linux/ARM64, flags is a struct with a .TYPE field
    const flags: posix.MAP = .{
        .TYPE = .SHARED,
    };

    _ = posix.mmap(
        null,
        stat.size,
        prot,
        flags,
        file.handle,
        0,
    ) catch return -3;

    return 0;
}

export fn scan_entropy() f32 {
    return 0.98; 
}
