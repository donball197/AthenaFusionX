// src/wasm_runtime.rs — v1.2.1 [LINEAR MEMORY BRIDGE + ARMOR]
use anyhow::{anyhow, Result};
use wasmtime::*;

const FUEL_CAP: u64 = 50_000_000;

pub struct WasmRuntime;

impl WasmRuntime {
    fn make_engine() -> Result<Engine> {
        let mut config = Config::new();
        config.consume_fuel(true);
        Ok(Engine::new(&config)?)
    }

    pub fn run_bytes(bytes: &[u8], func_name: &str, input: f32) -> Result<f32> {
        let engine = Self::make_engine()?;
        let module = Module::from_binary(&engine, bytes)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(FUEL_CAP)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<f32, f32>(&mut store, func_name)?;
        Ok(func.call(&mut store, input)?)
    }

    // ⚡ NEW: The Linear Memory Bridge
    pub fn run_with_buffer(bytes: &[u8], func_name: &str, data: &[u8]) -> Result<f32> {
        let engine = Self::make_engine()?;
        let module =
            Module::from_binary(&engine, bytes).map_err(|e| anyhow!("WASM_LOAD_FAULT: {}", e))?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(FUEL_CAP)?;
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| anyhow!("WASM_INST_FAULT: {}", e))?;

        // 1. Grab the WASM linear memory
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow!("WASM module must export memory"))?;

        // 2. Ask the WASM cell where its buffer is
        let get_ptr = instance
            .get_typed_func::<(), i32>(&mut store, "get_buffer_ptr")
            .map_err(|e| anyhow!("Missing get_buffer_ptr export: {}", e))?;
        let ptr = get_ptr.call(&mut store, ())? as usize;

        // ⚡ ARMOR INJECTED: Prevent sandbox overflow
        if data.len() > 65_536 {
            return Err(anyhow!(
                "VMO payload ({} bytes) exceeds the 64KB WASM sandbox buffer.",
                data.len()
            ));
        }

        // 3. Write our host data directly into the WASM sandbox
        memory
            .write(&mut store, ptr, data)
            .map_err(|e| anyhow!("WASM_MEMORY_WRITE_FAULT: {}", e))?;

        // 4. Trigger the function, passing only the length
        let func = instance
            .get_typed_func::<i32, f32>(&mut store, func_name)
            .map_err(|e| anyhow!("WASM_FUNC_MISSING [{}]: {}", func_name, e))?;

        Ok(func.call(&mut store, data.len() as i32)?)
    }
}
