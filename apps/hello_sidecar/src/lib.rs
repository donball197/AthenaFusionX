#[no_mangle]
pub extern "C" fn _start() {
    println!(">> [WASM CELL] Hello from the WebAssembly Sandbox!");
}
