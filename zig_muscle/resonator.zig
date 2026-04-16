/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


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
