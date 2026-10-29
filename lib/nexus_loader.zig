const std = @import("std");
const core = @import("safetensors.zig");

const EngineContext = struct {
    gpa: std.heap.GeneralPurposeAllocator(.{}),
    mapped: []const u8,
    st: core.SafeTensors,
};

pub const CTensorView = extern struct {
    data_ptr: [*]const u8,
    byte_size: u64,
    dtype_code: u8,
    shape_ptr: [*]const i64,
    shape_len: usize,
    internal_view: ?*anyopaque,
};

export fn nexus_dtype_element_size(dtype_code: u8) callconv(.c) u8 {
    if (dtype_code > @intFromEnum(core.DType.BOOL)) return 0;
    const dt: core.DType = @enumFromInt(dtype_code);
    return dt.elementSize();
}

export fn nexus_st_init(file_path: [*:0]const u8) callconv(.c) ?*anyopaque {
    const path = std.mem.span(file_path);

    const ctx = std.heap.c_allocator.create(EngineContext) catch return null;
    ctx.gpa = .{};
    const allocator = ctx.gpa.allocator();

    const file = std.fs.cwd().openFile(path, .{ .mode = .read_only }) catch {
        _ = ctx.gpa.deinit();
        std.heap.c_allocator.destroy(ctx);
        return null;
    };
    defer file.close();

    const stat = file.stat() catch {
        _ = ctx.gpa.deinit();
        std.heap.c_allocator.destroy(ctx);
        return null;
    };
    const file_size = stat.size;
    if (file_size == 0) {
        _ = ctx.gpa.deinit();
        std.heap.c_allocator.destroy(ctx);
        return null;
    }

    const mapped = std.posix.mmap(
        null,
        file_size,
        std.posix.PROT.READ,
        .{ .TYPE = .SHARED },
        file.handle,
        0,
    ) catch {
        _ = ctx.gpa.deinit();
        std.heap.c_allocator.destroy(ctx);
        return null;
    };

    ctx.mapped = mapped;

    ctx.st = core.SafeTensors.init(ctx.mapped, allocator) catch {
        std.posix.munmap(@alignCast(ctx.mapped));
        _ = ctx.gpa.deinit();
        std.heap.c_allocator.destroy(ctx);
        return null;
    };

    return ctx;
}

export fn nexus_st_get(
    handle: ?*anyopaque,
    name: [*:0]const u8,
    out_view: *CTensorView,
) callconv(.c) bool {
    const ctx: *EngineContext = @ptrCast(@alignCast(handle orelse return false));
    const name_slice = std.mem.span(name);

    const view = ctx.st.get(name_slice) catch return false;

    const view_ptr = ctx.st.allocator.create(core.TensorView) catch {
        ctx.st.freeView(view);
        return false;
    };
    view_ptr.* = view;

    out_view.* = .{
        .data_ptr = view.data.ptr,
        .byte_size = view.byteSize(),
        .dtype_code = @intFromEnum(view.dtype),
        .shape_ptr = view.shape.ptr,
        .shape_len = view.shape.len,
        .internal_view = view_ptr,
    };

    return true;
}

export fn nexus_st_free_view(handle: ?*anyopaque, view: *CTensorView) callconv(.c) void {
    const ctx: *EngineContext = @ptrCast(@alignCast(handle orelse return));
    if (view.internal_view) |ptr| {
        const view_ptr: *core.TensorView = @ptrCast(@alignCast(ptr));
        ctx.st.freeView(view_ptr.*);
        ctx.st.allocator.destroy(view_ptr);
        view.internal_view = null;
    }
}

export fn nexus_st_deinit(handle: ?*anyopaque) callconv(.c) void {
    const ctx: *EngineContext = @ptrCast(@alignCast(handle orelse return));
    ctx.st.deinit();
    std.posix.munmap(@alignCast(ctx.mapped));
    _ = ctx.gpa.deinit();
    std.heap.c_allocator.destroy(ctx);
}
