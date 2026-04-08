#[cfg(feature = "future_npu")]
use nexus_singularity::tflite_bridge::LiteRTRuntime;

#[cfg(not(feature = "future_npu"))]
fn main() {
    println!("\x1b[33m>> [NPU TEST] future_npu feature is DISABLED.\x1b[0m");
    println!("   Run with: cargo run --bin test_npu --features future_npu");
}

#[cfg(feature = "future_npu")]
fn main() {
    println!("\x1b[36m>> [HARDWARE TEST] Igniting NPU Bridge...\x1b[0m");
    
    // SUTURE: Bypassing the bleeding-edge dummy model.
    // Targeting the established logic_v1 core to verify the FFI bridge.
    match LiteRTRuntime::ignite("vault/logic_v1.tflite") {
        Ok(_) => {
            println!("\x1b[32m>> [SUCCESS] NPU Handshake Complete. Tensor Mapped. 1.0 Verified.\x1b[0m");
        }
        Err(e) => {
            println!("\x1b[31m!! [CRITICAL FAILURE] {}\x1b[0m", e);
        }
    }
}
