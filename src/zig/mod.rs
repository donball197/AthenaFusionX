const ZIG_CELL_WASM: &[u8] = include_bytes!("zig-out/bin/nexus_zig_cell.wasm");

pub fn resonance_from_vmo(handle: u32, size: usize) -> f32 {
    let mut data = vec![0u8; size.min(4096)];
    unsafe {
        if crate::kernel::zx_vmo_read_native(handle, 0, data.as_mut_ptr(), data.len()) != 0 {
            return 0.0;
        }
    }
    resonance_from_buffer(&data)
}

pub fn resonance_from_buffer(data: &[u8]) -> f32 {
    match crate::wasm_runtime::WasmRuntime::run_with_buffer(
        ZIG_CELL_WASM, 
        "calculate_resonance_e2b",
        data,
    ) {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!("Zig WASM cell failed: {}", e);
            0.0
        }
    }
}
