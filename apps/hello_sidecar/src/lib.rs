// Only compile this entry point when targeting WebAssembly
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn _start() {
    println!(">> [WASM CELL] Hello from the WebAssembly Sandbox!");
}

// Provide a harmless dummy function for the ARM64 host to keep the crate valid
#[cfg(not(target_arch = "wasm32"))]
pub fn host_dummy() {
    // This prevents the linker collision during workspace tests
}
