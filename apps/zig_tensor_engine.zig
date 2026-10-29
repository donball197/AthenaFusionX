const std = @import("std");

fn matmul(comptime M: usize, comptime K: usize, comptime N: usize, A: *const [M][K]f32, B: *const [K][N]f32, C: *[M][N]f32) void {
    for (0..M) |i| {
        for (0..N) |j| {
            C[i][j] = 0.0;
            for (0..K) |l| {
                C[i][j] += A[i][l] * B[l][j];
            }
        }
    }
}

fn rmsnorm(comptime Len: usize, x: *const [Len]f32, y: *[Len]f32, epsilon: f32) void {
    var rms: f32 = 0.0;
    for (x) |val| {
        rms += val * val;
    }
    rms = @sqrt((rms / @as(f32, @floatFromInt(Len))) + epsilon);
    for (0..Len) |i| {
        y[i] = x[i] / rms;
    }
}

pub fn main() !void {
    const A = [2][3]f32{
        [_]f32{ 1.0, 2.0, 3.0 },
        [_]f32{ 4.0, 5.0, 6.0 },
    };
    const B = [3][2]f32{
        [_]f32{ 7.0, 8.0 },
        [_]f32{ 9.0, 10.0 },
        [_]f32{ 11.0, 12.0 },
    };
    var C: [2][2]f32 = undefined;

    matmul(2, 3, 2, &A, &B, &C);

    std.debug.print("Matrix Multiplication Result (2x2):\n", .{});
    for (C) |row| {
        for (row) |val| {
            std.debug.print("{d:.2} ", .{val});
        }
        std.debug.print("\n", .{});
    }

    const x = [5]f32{ 1.0, 2.0, 3.0, 4.0, 5.0 };
    var y: [5]f32 = undefined;
    rmsnorm(5, &x, &y, 1e-5);

    std.debug.print("\nRMSNorm Result:\n", .{});
    for (y) |val| {
        std.debug.print("{d:.4} ", .{val});
    }
    std.debug.print("\n", .{});
}
