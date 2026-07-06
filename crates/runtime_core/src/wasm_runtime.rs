use anyhow::{anyhow, Result};
use wasmtime::*;

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
        store.set_fuel(50_000_000)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<f32, f32>(&mut store, func_name)?;
        Ok(func.call(&mut store, input)?)
    }

    pub fn run_with_buffer(bytes: &[u8], func_name: &str, data: &[u8]) -> Result<f32> {
        let engine = Self::make_engine()?;
        let module =
            Module::from_binary(&engine, bytes).map_err(|e| anyhow!("WASM_LOAD_FAULT: {}", e))?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(50_000_000)?;
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| anyhow!("WASM_INST_FAULT: {}", e))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow!("WASM module must export memory"))?;

        let get_ptr = instance
            .get_typed_func::<(), i32>(&mut store, "get_buffer_ptr")
            .map_err(|e| anyhow!("Missing get_buffer_ptr export: {}", e))?;
        let ptr = get_ptr.call(&mut store, ())? as usize;

        if data.len() > 65_536 {
            return Err(anyhow!("VMO payload exceeds 64KB sandbox."));
        }

        // WASMtime 43+ API: Use .write() directly on the memory slice
        memory
            .write(&mut store, ptr, data)
            .map_err(|e| anyhow!("WASM_MEMORY_WRITE_FAULT: {}", e))?;

        let func = instance
            .get_typed_func::<i32, f32>(&mut store, func_name)
            .map_err(|e| anyhow!("WASM_FUNC_MISSING: {}", e))?;

        Ok(func.call(&mut store, data.len() as i32)?)
    }
}
