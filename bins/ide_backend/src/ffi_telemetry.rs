// FFI Telemetry Wrapper Module
// Intercepts process_cognitive_task return codes and maps them to OpenTelemetry span attributes.

pub struct FfiTelemetryWrapper;

impl FfiTelemetryWrapper {
    pub fn intercept_cognitive_task<F, R>(task_name: &str, f: F) -> R
    where
        F: FnOnce() -> (i32, R),
    {
        // Execute the underlying FFI call
        let (return_code, result) = f();
        
        // Map return code to error category and semantic attributes
        let error_category = match return_code {
            0 => "SUCCESS",
            1..=99 => "VALIDATION_ERROR",
            100..=199 => "INTERNAL_ERROR",
            _ => "UNKNOWN_ERROR",
        };

        // Emit via telemetry bridge pipeline if available
        // (Integrating with telemetry_bridge.rs structural logging)
        println!("[TelemetryBridge] span=process_cognitive_task task={} ffi.return_code={} ffi.error_category={}", 
            task_name, return_code, error_category);

        result
    }
}
