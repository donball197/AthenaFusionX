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


const std = @import("std"); pub fn main() !void { const f = try std.fs.openFileAbsolute("/sys/class/hwmon/hwmon0/temp1_input", .{}); defer f.close(); var b: [16]u8 = undefined; const n = try f.readAll(&b); std.io.getStdOut().writer().print("{s}", .{b[0..n]}) catch {}; }