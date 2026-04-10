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
const build_options = @import("build_options"); // SUTURE: Import the DNA matrix

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const log_file = try std.fs.cwd().createFile("nexus_watchdog.log", .{ .truncate = false });
    defer log_file.close();
    try log_file.seekFromEnd(0);
    const writer = log_file.writer();

    const api_key = std.process.getEnvVarOwned(allocator, "NEXUS_API_KEY") catch "MISSING_KEY";
    defer if (!std.mem.eql(u8, api_key, "MISSING_KEY")) allocator.free(api_key);

    try writer.print("\n--- NEXUS v0.2.6 (RESILIENT) START: {d} ---\n", .{std.time.timestamp()});
    
    if (build_options.is_duet2) try writer.print(">> [HOST] Lenovo Duet 2 (Crostini) Detected.\n", .{});
    if (build_options.is_moto) try writer.print(">> [HOST] Motorola Edge (Termux) Detected.\n", .{});

    while (true) {
        var child = std.process.Child.init(&[_][]const u8{ "git", "log", "-1", "--pretty=format:HASH:%H MSG:%B" }, allocator);
        child.stdout_behavior = .Pipe;
        try child.spawn();
        const msg = try child.stdout.?.reader().readAllAlloc(allocator, 8192);
        defer allocator.free(msg);
        _ = try child.wait();

        const url = try std.fmt.allocPrint(allocator, "https://generativelanguage.googleapis.com/v1beta/models/gemma-3-27b-it:generateContent?key={s}", .{api_key});
        defer allocator.free(url);
        const payload = try std.fmt.allocPrint(allocator, "{{\"contents\": [{{ \"parts\":[{{ \"text\": \"Audit DNA. Provide git patch between PATCH_START and PATCH_END: {s}\" }}]}}]}}", .{msg});
        defer allocator.free(payload);

        var api_child = std.process.Child.init(&[_][]const u8{ "curl", "-s", "-X", "POST", url, "-H", "Content-Type: application/json", "-d", payload }, allocator);
        api_child.stdout_behavior = .Pipe;
        try api_child.spawn();
        const response = try api_child.stdout.?.reader().readAllAlloc(allocator, 64000);
        defer allocator.free(response);
        _ = try api_child.wait();

        if (std.mem.indexOf(u8, response, "PATCH_START")) |start_idx| {
            if (std.mem.indexOf(u8, response, "PATCH_END")) |end_idx| {
                const patch = response[start_idx + 11 .. end_idx];
                try writer.print("[{d}] SENTINEL: AUDIT RECEIVED (STAGED). Patch stored in log.\n", .{std.time.timestamp()});

                const patch_file = try std.fs.cwd().createFile("repair.patch", .{});
                defer patch_file.close();
                try patch_file.writeAll(patch);

                // SUTURE: Hardware-aware notification block. Will not crash on Crostini.
                if (build_options.is_moto) {
                    var notify = std.process.Child.init(&[_][]const u8{ "termux-notification", "-t", "NEXUS RESILIENT", "-c", "High-Fidelity Patch Ready for Review." }, allocator);
                    _ = notify.spawnAndWait() catch {}; 
                } else if (build_options.is_duet2) {
                    // Fallback for Chromebook Native Linux via libnotify (if installed)
                    var notify = std.process.Child.init(&[_][]const u8{ "notify-send", "NEXUS RESILIENT", "High-Fidelity Patch Ready for Review." }, allocator);
                    _ = notify.spawnAndWait() catch {};
                }
            }
        }
        std.time.sleep(60 * std.time.ns_per_s);
    }
}
