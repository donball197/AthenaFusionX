const std = @import("std");

pub const DType = enum(u8) {
    F64 = 0,
    F32 = 1,
    F16 = 2,
    BF16 = 3,
    I64 = 4,
    I32 = 5,
    I16 = 6,
    I8 = 7,
    U8 = 8,
    BOOL = 9,

    pub fn fromString(s: []const u8) !DType {
        if (std.mem.eql(u8, s, "F64")) return .F64;
        if (std.mem.eql(u8, s, "F32")) return .F32;
        if (std.mem.eql(u8, s, "F16")) return .F16;
        if (std.mem.eql(u8, s, "BF16")) return .BF16;
        if (std.mem.eql(u8, s, "I64")) return .I64;
        if (std.mem.eql(u8, s, "I32")) return .I32;
        if (std.mem.eql(u8, s, "I16")) return .I16;
        if (std.mem.eql(u8, s, "I8")) return .I8;
        if (std.mem.eql(u8, s, "U8")) return .U8;
        if (std.mem.eql(u8, s, "BOOL")) return .BOOL;
        return error.UnknownDType;
    }

    pub fn elementSize(self: DType) u8 {
        return switch (self) {
            .F64, .I64 => 8,
            .F32, .I32 => 4,
            .F16, .BF16, .I16 => 2,
            .I8, .U8, .BOOL => 1,
        };
    }
};

pub const TensorView = struct {
    name: []const u8,
    dtype: DType,
    shape: []const i64,
    data: []const u8,
    data_begin: u64,
    data_end: u64,

    pub fn byteSize(self: TensorView) u64 {
        return self.data_end - self.data_begin;
    }

    pub fn numel(self: TensorView) u64 {
        var n: u64 = 1;
        for (self.shape) |d| n *= @intCast(d);
        return n;
    }
};

pub const SafeTensors = struct {
    mapped: []const u8,
    header_size: u64,
    data_offset: u64,
    parsed: std.json.Parsed(std.json.Value),
    allocator: std.mem.Allocator,

    pub fn init(mapped: []const u8, allocator: std.mem.Allocator) !SafeTensors {
        if (mapped.len < 8) return error.FileTooSmall;

        const header_size = std.mem.readInt(u64, mapped[0..8], .little);
        if (header_size == 0 or header_size > 100_000_000) return error.InvalidHeaderSize;
        if (8 + header_size > mapped.len) return error.HeaderOverflow;

        const header_json = mapped[8 .. 8 + header_size];
        if (header_json[0] != '{') return error.InvalidHeader;

        var parsed = try std.json.parseFromSlice(std.json.Value, allocator, header_json, .{});
        errdefer parsed.deinit();

        if (parsed.value != .object) {
            parsed.deinit();
            return error.ExpectedObject;
        }

        return .{
            .mapped = mapped,
            .header_size = header_size,
            .data_offset = 8 + header_size,
            .parsed = parsed,
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *SafeTensors) void {
        self.parsed.deinit();
    }

    pub fn get(self: *const SafeTensors, name: []const u8) !TensorView {
        const root = self.parsed.value.object;
        const entry = root.get(name) orelse return error.TensorNotFound;
        if (entry != .object) return error.InvalidTensorEntry;

        const obj = entry.object;

        const dtype_str = (obj.get("dtype") orelse return error.MissingDtype).string;
        const dtype = try DType.fromString(dtype_str);

        const shape_val = obj.get("shape") orelse return error.MissingShape;
        if (shape_val != .array) return error.InvalidShape;

        var shape = try self.allocator.alloc(i64, shape_val.array.items.len);
        errdefer self.allocator.free(shape);
        for (shape_val.array.items, 0..) |v, i| {
            shape[i] = v.integer;
        }

        const offsets_val = obj.get("data_offsets") orelse return error.MissingOffsets;
        if (offsets_val != .array or offsets_val.array.items.len != 2)
            return error.InvalidOffsets;

        const begin: u64 = @intCast(offsets_val.array.items[0].integer);
        const end: u64 = @intCast(offsets_val.array.items[1].integer);

        if (end < begin or self.data_offset + end > self.mapped.len) {
            self.allocator.free(shape);
            return error.InvalidOffsetsRange;
        }

        const data = self.mapped[self.data_offset + begin .. self.data_offset + end];

        return TensorView{
            .name = name,
            .dtype = dtype,
            .shape = shape,
            .data = data,
            .data_begin = begin,
            .data_end = end,
        };
    }

    pub fn freeView(self: *const SafeTensors, view: TensorView) void {
        self.allocator.free(view.shape);
    }
};
