const std = @import("std");

// Compact Swarm Unit Definition
pub const SwarmUnit = struct {
    id: u32,
    status: u8,   // 0: Offline, 1: Idle, 2: Active
    load: u8,     // 0-100 percentage
};

// Fixed memory allocation for 68 units (The Hive)
var hive_memory: [68]SwarmUnit = undefined;

export fn init_hive() void {
    for (&hive_memory, 0..) |*unit, i| {
        unit.* = SwarmUnit{
            .id = @intCast(i),
            .status = 1, // Start in Idle
            .load = 0,
        };
    }
}

export fn get_hive_ptr() [*]SwarmUnit {
    return &hive_memory;
}

export fn update_unit(id: u32, status: u8, load: u8) void {
    if (id < 68) {
        hive_memory[id].status = status;
        hive_memory[id].load = load;
    }
}
