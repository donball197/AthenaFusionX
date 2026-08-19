use anyhow::{anyhow, Result};
use std::path::Path;
use std::time::Instant;
use std::fs::OpenOptions;
use std::io::Write;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;
use serde_json::json;

pub struct HostState {
    pub wasi: WasiP1Ctx,
}

fn main() -> Result<()> {
    let wasm_path = Path::new("sidecars/hello_sidecar.wasm");
    if !wasm_path.exists() {
        return Err(anyhow!("CRITICAL: Sidecar artifact missing at sidecars/hello_sidecar.wasm"));
    }

    let mut config = wasmtime::Config::new();
    config.cranelift_opt_level(wasmtime::OptLevel::Speed);

    let engine = Engine::new(&config)?;
    let mut linker = Linker::new(&engine);
    
    // Wasmtime v45 uses p1 instead of preview1
    p1::add_to_linker_sync(&mut linker, |state: &mut HostState| &mut state.wasi)?;

    let module = Module::from_file(&engine, wasm_path)?;
    let wasi_ctx = WasiCtxBuilder::new().inherit_stdout().inherit_stderr().build_p1();
    let mut store = Store::new(&engine, HostState { wasi: wasi_ctx });
    
    let instance = linker.instantiate(&mut store, &module)?;

    let simulated_turbovec_output = r#"[{"id": "node_004", "similarity": 0.9856, "context": "Sanitize path traversals via standard absolute canonical mappings."}]"#;
    let payload_bytes = simulated_turbovec_output.as_bytes();
    
    let memory = instance.get_export(&mut store, "memory").and_then(|e| e.into_memory()).ok_or_else(|| anyhow!("WASM Memory missing"))?;
    let get_buffer_ptr_fn = instance.get_typed_func::<(), u32>(&mut store, "get_buffer_ptr")?;
    let process_task_fn = instance.get_typed_func::<u32, i32>(&mut store, "process_cognitive_task")?;
    
    let ptr = get_buffer_ptr_fn.call(&mut store, ())? as usize;
    memory.write(&mut store, ptr, payload_bytes).map_err(|_| anyhow!("Zero-copy boundary exceeded"))?;

    // ⏱️ START TELEMETRY PROFILING
    let start_time = Instant::now();
    let result_code = process_task_fn.call(&mut store, payload_bytes.len() as u32)?;
    let exec_duration = start_time.elapsed();

    println!("[✅ SYSTEM STATUS] Host-to-sandbox processing pass successful. Exit Code: {}", result_code);
    
    // 📝 WRITE JSON LEDGER
    let report = json!({
        "timestamp_utc": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        "event": "wasm_sidecar_execution",
        "exit_code": result_code,
        "metrics": {
            "execution_time_microseconds": exec_duration.as_micros(),
            "payload_injected_bytes": payload_bytes.len(),
        }
    });

    let mut file = OpenOptions::new().create(true).append(true).open("sandbox_ledger.json")?;
    writeln!(file, "{}", report)?;

    Ok(())
}
