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

/// THERMAL-PULSE: Hardware-level temperature extraction.
export fn extract_thermal_pulse_raw() i32 {
    const paths = [_][]const u8{
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/thermal/thermal_zone1/temp",
        "/sys/class/hwmon/hwmon0/temp1_input",
        "/sys/class/hwmon/hwmon1/temp1_input",
    };
    var buf: [16]u8 = undefined;
    for (paths) |path| {
        const file = std.fs.openFileAbsolute(path, .{}) catch continue;
        defer file.close();
        const bytes_read = file.readAll(&buf) catch continue;
        if (bytes_read == 0) continue;
        const slice = std.mem.trim(u8, buf[0..bytes_read], " \n\r\t");
        return std.fmt.parseInt(i32, slice, 10) catch continue;
    }
    return 0;
}

/// ALPHA-SCOUT: High-speed payload extraction.
export fn extract_payload_strands(
    input_ptr: [*]const u8,
    input_len: usize,
    target_ptr: [*]const u8,
    target_len: usize,
    output_ptr: [*]u8,
    output_max: usize,
) usize {
    if (input_len == 0 or target_len == 0 or output_max == 0) return 0;
    const input = input_ptr[0..input_len];
    const target = target_ptr[0..target_len];
    if (std.mem.indexOf(u8, input, target)) |start_idx| {
        const data_start = start_idx + target_len;
        var end_idx = data_start;
        while (end_idx < input_len and input[end_idx] != '"' and input[end_idx] != '<' and input[end_idx] != '\n') {
            end_idx += 1;
        }
        const extracted_len = end_idx - data_start;
        if (extracted_len > 0 and extracted_len <= output_max) {
            std.mem.copyForwards(u8, output_ptr[0..extracted_len], input[data_start..end_idx]);
            return extracted_len;
        }
    }
    return 0;
}
