const BUFFER_CAPACITY: usize = 65536;
static mut SHARED_BUFFER: [u8; BUFFER_CAPACITY] = [0; BUFFER_CAPACITY];

#[no_mangle]
pub extern "C" fn get_buffer_ptr() -> *mut u8 {
    core::ptr::addr_of_mut!(SHARED_BUFFER) as *mut u8
}

#[no_mangle]
pub extern "C" fn process_cognitive_task(payload_len: u32) -> i32 {
    let len = payload_len as usize;
    if len > BUFFER_CAPACITY {
        return 1;
    }

    let payload =
        unsafe { std::slice::from_raw_parts(core::ptr::addr_of!(SHARED_BUFFER) as *const u8, len) };

    match std::str::from_utf8(payload) {
        Ok(context_str) => {
            println!(">> [WASM GUEST] Validated {} byte payload.", len);
            let preview = if context_str.len() > 100 {
                format!("{}...", &context_str[..100])
            } else {
                context_str.to_string()
            };
            println!(">> [WASM GUEST] Context Received: {}", preview);
            0
        }
        Err(_) => {
            eprintln!(">> [WASM GUEST ERROR] Invalid UTF-8.");
            2
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() {}
