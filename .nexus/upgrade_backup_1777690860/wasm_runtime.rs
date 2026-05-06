use anyhow::{anyhow, Result};
use std::sync::LazyLock;
use wasmtime::*;

const FUEL_CAP: u64 = 50_000_000;

// ⚡ FIX 2: Global static Engine eliminates instantiation RAM overhead
static WASM_ENGINE: LazyLock<Engine> = LazyLock::new(|| {
    let mut config = Config::new();
    config.consume_fuel(true);
    Engine::new(&config).expect("WASM_ENGINE initialization failed")
});

pub struct WasmRuntime;

impl WasmRuntime {
    pub fn run_bytes(bytes: &[u8], func_name: &str, input: f32) -> Result<f32> {
        let engine = WASM_ENGINE.clone();
        let module = Module::from_binary(&engine, bytes)
            .map_err(|e| anyhow!("WASM_LOAD_FAULT [{}]: {}", func_name, e))?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(FUEL_CAP)?;
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| anyhow!("WASM_INST_FAULT [{}]: {}", func_name, e))?;
        let func = instance
            .get_typed_func::<f32, f32>(&mut store, func_name)
            .map_err(|e| anyhow!("WASM_FUNC_MISSING [{}]: {}", func_name, e))?;
        func.call(&mut store, input).map_err(|e| anyhow!("WASM_EXEC_FAULT [{}]: {}", func_name, e))
    }

    pub fn run_f64(bytes: &[u8], func_name: &str, input: f64) -> Result<f64> {
        let engine = WASM_ENGINE.clone();
        let module = Module::from_binary(&engine, bytes)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(FUEL_CAP)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<f64, f64>(&mut store, func_name)?;
        Ok(func.call(&mut store, input)?)
    }

    pub fn run_i32(bytes: &[u8], func_name: &str, input: i32) -> Result<i32> {
        let engine = WASM_ENGINE.clone();
        let module = Module::from_binary(&engine, bytes)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(FUEL_CAP)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<i32, i32>(&mut store, func_name)?;
        Ok(func.call(&mut store, input)?)
    }

    pub fn list_exports(bytes: &[u8]) -> Result<Vec<String>> {
        let engine = WASM_ENGINE.clone();
        let module = Module::from_binary(&engine, bytes)?;
        Ok(module
            .exports()
            .filter(|e| matches!(e.ty(), ExternType::Func(_)))
            .map(|e| e.name().to_string())
            .collect())
    }

    pub fn validate(bytes: &[u8]) -> Result<()> {
        let engine = WASM_ENGINE.clone();
        Module::from_binary(&engine, bytes).map(|_| ()).map_err(|e| anyhow!("WASM_INVALID: {}", e))
    }
}
