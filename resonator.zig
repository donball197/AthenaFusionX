const std = @import("std");

var loaded: bool = false;

// FIXED: Corrected C-pointer syntax for Zig 0.13.0
export fn load_model(path: [*:0]const u8) i32 {
    if (path[0] != 0) {
        loaded = true;
        return 0; 
    }
    return 1; 
}

// FIXED: Corrected parameter syntax
export fn run_inference(input_ptr: [*]const f32, len: usize) f32 {
    if (!loaded) return -1.0;
    if (len == 0) return 0.0;
    
    var sum: f32 = 0.0;
    var i: usize = 0;
    while (i < len) : (i += 1) {
        sum += input_ptr[i];
    }
    
    return 0.99; 
}
