#!/bin/bash
# forge_neon_kernel.sh - Project Nexus Singularity v0.2.4
# Generates complete Zero-Copy Zig/Rust FFI + ARM64 NEON MatMul substrate

set -euo pipefail

echo ">> [NEXUS] Forging complete Zero-Copy + NEON substrate..."

mkdir -p lib
mkdir -p crates/nexus_edge_minion/src

# ==============================================================================
# 0. CORE ZIG LOGIC (lib/safetensors.zig) - Zig 0.15 compatible
# ==============================================================================
echo ">> [NEXUS] Writing lib/safetensors.zig..."
cat << 'EOF' > lib/safetensors.zig
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
EOF

# ==============================================================================
# 1. ZIG FFI BRIDGE (lib/nexus_loader.zig) - Zig 0.15 compatible
# ==============================================================================
echo ">> [NEXUS] Writing lib/nexus_loader.zig..."
cat << 'EOF' > lib/nexus_loader.zig
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
        std.posix.munmap(ctx.mapped);
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
    std.posix.munmap(ctx.mapped);
    _ = ctx.gpa.deinit();
    std.heap.c_allocator.destroy(ctx);
}
EOF

# ==============================================================================
# 2. RUST SAFE WRAPPER + as_bf16 helper
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/safetensors.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/safetensors.rs
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::slice;

#[repr(C)]
pub struct CTensorView {
    pub data_ptr: *const u8,
    pub byte_size: u64,
    pub dtype_code: u8,
    pub shape_ptr: *const i64,
    pub shape_len: usize,
    internal_view: *mut c_void,
}

extern "C" {
    fn nexus_st_init(file_path: *const c_char) -> *mut c_void;
    fn nexus_st_get(handle: *mut c_void, name: *const c_char, out_view: *mut CTensorView) -> bool;
    fn nexus_st_free_view(handle: *mut c_void, view: *mut CTensorView);
    fn nexus_st_deinit(handle: *mut c_void);
    fn nexus_dtype_element_size(dtype_code: u8) -> u8;
}

pub struct EdgeLoader {
    handle: *mut c_void,
}

impl EdgeLoader {
    pub fn new(path: &str) -> Result<Self, &'static str> {
        let c_path = CString::new(path).map_err(|_| "path contained interior nul")?;
        let handle = unsafe { nexus_st_init(c_path.as_ptr()) };
        if handle.is_null() {
            return Err("nexus_st_init failed");
        }
        Ok(Self { handle })
    }

    pub fn get_tensor<'a>(&'a self, name: &str) -> Result<TensorSlice<'a>, &'static str> {
        let c_name = CString::new(name).map_err(|_| "tensor name contained interior nul")?;
        let mut c_view = CTensorView {
            data_ptr: ptr::null(),
            byte_size: 0,
            dtype_code: 0,
            shape_ptr: ptr::null(),
            shape_len: 0,
            internal_view: ptr::null_mut(),
        };

        let ok = unsafe { nexus_st_get(self.handle, c_name.as_ptr(), &mut c_view) };
        if !ok {
            return Err("tensor not found or internal error");
        }

        let data = unsafe { slice::from_raw_parts(c_view.data_ptr, c_view.byte_size as usize) };
        let shape = unsafe { slice::from_raw_parts(c_view.shape_ptr, c_view.shape_len) };
        let dtype = c_view.dtype_code;

        Ok(TensorSlice {
            engine: self,
            c_view,
            data,
            shape,
            dtype,
        })
    }
}

impl Drop for EdgeLoader {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { nexus_st_deinit(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

pub struct TensorSlice<'a> {
    engine: &'a EdgeLoader,
    c_view: CTensorView,
    pub data: &'a [u8],
    pub shape: &'a [i64],
    pub dtype: u8,
}

impl<'a> Drop for TensorSlice<'a> {
    fn drop(&mut self) {
        unsafe {
            nexus_st_free_view(self.engine.handle, &mut self.c_view);
        }
    }
}

impl<'a> TensorSlice<'a> {
    pub fn numel(&self) -> u64 {
        self.shape.iter().fold(1u64, |acc, &d| acc * d as u64)
    }

    pub fn element_size(&self) -> u8 {
        unsafe { nexus_dtype_element_size(self.dtype) }
    }

    /// Zero-copy cast to bf16
    pub fn as_bf16(&self) -> Result<TypedTensor<'a, half::bf16>, &'static str> {
        if self.dtype != 3 {
            return Err("tensor is not BF16");
        }
        if self.data.len() % 2 != 0 {
            return Err("BF16 data length is not a multiple of 2");
        }
        let ptr = self.data.as_ptr() as *const half::bf16;
        let len = self.data.len() / 2;
        let data = unsafe { slice::from_raw_parts(ptr, len) };

        Ok(TypedTensor {
            data,
            shape: self.shape,
        })
    }
}

pub struct TypedTensor<'a, T> {
    pub data: &'a [T],
    pub shape: &'a [i64],
}
EOF

# ==============================================================================
# 3. NEON KERNEL
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/neon_math.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/neon_math.rs
use std::arch::aarch64::*;
use half::bf16;

/// ARM64 NEON Matrix-Vector Multiplication (y = W * x)
/// W: [M, N] bf16   x: [N] bf16   y: [M] f32
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn matvec_bf16_to_f32_neon(
    w: &[bf16],
    x: &[bf16],
    y: &mut [f32],
    m: usize,
    n: usize,
) {
    assert_eq!(w.len(), m * n);
    assert_eq!(x.len(), n);
    assert_eq!(y.len(), m);

    let shift_amount = vdupq_n_s32(16);

    for i in 0..m {
        let mut acc0 = vdupq_n_f32(0.0);
        let mut acc1 = vdupq_n_f32(0.0);
        let row_offset = i * n;
        let mut j = 0;

        while j + 8 <= n {
            let vw = vld1q_u16(w.as_ptr().add(row_offset + j) as *const u16);
            let vx = vld1q_u16(x.as_ptr().add(j) as *const u16);

            let vw_low  = vreinterpretq_f32_u32(vshlq_u32(vmovl_u16(vget_low_u16(vw)),  shift_amount));
            let vx_low  = vreinterpretq_f32_u32(vshlq_u32(vmovl_u16(vget_low_u16(vx)),  shift_amount));
            let vw_high = vreinterpretq_f32_u32(vshlq_u32(vmovl_u16(vget_high_u16(vw)), shift_amount));
            let vx_high = vreinterpretq_f32_u32(vshlq_u32(vmovl_u16(vget_high_u16(vx)), shift_amount));

            acc0 = vfmaq_f32(acc0, vw_low,  vx_low);
            acc1 = vfmaq_f32(acc1, vw_high, vx_high);

            j += 8;
        }

        let acc = vaddq_f32(acc0, acc1);
        let mut sum = vaddvq_f32(acc);

        while j < n {
            sum += w[row_offset + j].to_f32() * x[j].to_f32();
            j += 1;
        }

        y[i] = sum;
    }
}
EOF

# ==============================================================================
# 4. MAIN
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/main.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/main.rs
mod safetensors;
mod neon_math;

use half::bf16;
use safetensors::EdgeLoader;
use std::time::Instant;

fn main() {
    let cache_dir = std::env::var("NEXUS_CACHE_DIR").unwrap_or_else(|_| {
        "/mnt/chromeos/removable/Memorex USB/AthenaFusionX/gemma-4-e2b-weights".to_string()
    });
    let model_path = format!("{}/model.safetensors", cache_dir);

    println!(">> [MINION] Igniting Zero-Copy Loader...");
    let loader = EdgeLoader::new(&model_path).expect("Loader failed to ignite");

    // Change this key to a real BF16 tensor that exists in your file
    let weight_key = "model.audio_tower.layers.0.feed_forward1.ffw_layer_1.linear.weight";

    let raw_tensor = loader
        .get_tensor(weight_key)
        .unwrap_or_else(|_| panic!("Failed to locate {}", weight_key));

    let typed_w = raw_tensor.as_bf16().expect("tensor is not BF16");

    let m = typed_w.shape[0] as usize;
    let n = typed_w.shape[1] as usize;
    println!(">> [MINION] Substrate: {} ({}×{})", weight_key, m, n);

    let x_in = vec![bf16::from_f32(1.0); n];
    let mut y_out = vec![0.0f32; m];

    println!(">> [MINION] Executing ARM64 NEON MatMul...");
    let start = Instant::now();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon_math::matvec_bf16_to_f32_neon(typed_w.data, &x_in, &mut y_out, m, n);
    }

    let duration = start.elapsed();
    println!(">> [MINION] Done in {:?}", duration);
    println!("  y[0] = {}", y_out[0]);
    println!("  y[1] = {}", y_out[1]);
    println!("  Zero-copy + NEON path stable.");
}
EOF

# ==============================================================================
# 5. BUILD SHARED LIBRARY
# ==============================================================================
echo ">> [NEXUS] Compiling libnexus_loader.so (Zig 0.15)..."
rm -f lib/libnexus_loader.so

zig build-lib lib/nexus_loader.zig \
    -O ReleaseSmall \
    -dynamic \
    -fPIC \
    -lc \
    -femit-bin=lib/libnexus_loader.so

echo ""
echo ">> [NEXUS] All files generated and shared library built."
echo ""
echo "Next steps:"
echo "  1. Make sure Cargo.toml contains:  half = \"2.4\""
echo "  2. export LD_LIBRARY_PATH=\"\$(pwd)/lib:\$LD_LIBRARY_PATH\""
echo "  3. cargo build -p nexus_edge_minion --release"
echo "  4. cargo run  -p nexus_edge_minion --release"
echo ""
