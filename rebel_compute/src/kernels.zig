const std = @import("std");

// Specialized Kernel: Pattern matching in your 5MB logic file
// Using 'export' makes this function visible to Rust's FFI
export fn scan_patterns(data: [*]const u8, len: usize) i32 {
    var result: i32 = 0;
    const bytes = data[0..len];
    
    // The "Rebel" Loop: Manual SIMD-ready iteration
    for (bytes) |byte| {
        if (byte == 'S' or byte == 'I' or byte == 'D') {
            result += 1;
        }
    }
    return result;
}
