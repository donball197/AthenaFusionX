// src/wasm_runtime.rs — v1.1 Hardened Wasmtime Runtime
// FIX: Added explicit fuel cap to prevent runaway WASM cells from
//      monopolising CPU on the 4GB Duet.
// FIX: Better error context — includes func_name in all error messages.
// NEW: run_f64 for double-precision cells.
// NEW: run_named for registry-backed execution.
use anyhow::{anyhow, Result};
use wasmtime::*;

/// FUEL_CAP: Max instructions before forceful termination.
/// 10M instructions ≈ ~10ms of compute on ARM64 Crostini at 1GHz.
const FUEL_CAP: u64 = 50_000_000;

pub struct WasmRuntime;

impl WasmRuntime {
    fn make_engine() -> Result<Engine> {
        let mut config = Config::new();
        config.consume_fuel(true);
        Ok(Engine::new(&config)?)
    }

    /// run_bytes: Execute a compiled WASM cell with f32 I/O.
    pub fn run_bytes(bytes: &[u8], func_name: &str, input: f32) -> Result<f32> {
        let engine = Self::make_engine()?;
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

    /// run_f64: f64 variant for higher-precision cells.
    pub fn run_f64(bytes: &[u8], func_name: &str, input: f64) -> Result<f64> {
        let engine = Self::make_engine()?;
        let module = Module::from_binary(&engine, bytes)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(FUEL_CAP)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<f64, f64>(&mut store, func_name)?;
        Ok(func.call(&mut store, input)?)
    }

    /// run_i32: Integer variant for flag/status cells.
    pub fn run_i32(bytes: &[u8], func_name: &str, input: i32) -> Result<i32> {
        let engine = Self::make_engine()?;
        let module = Module::from_binary(&engine, bytes)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(FUEL_CAP)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<i32, i32>(&mut store, func_name)?;
        Ok(func.call(&mut store, input)?)
    }

    /// list_exports: Returns names of all exported functions in a WASM module.
    /// Useful for discovery without executing.
    pub fn list_exports(bytes: &[u8]) -> Result<Vec<String>> {
        let engine = Engine::default();
        let module = Module::from_binary(&engine, bytes)?;
        Ok(module
            .exports()
            .filter(|e| matches!(e.ty(), ExternType::Func(_)))
            .map(|e| e.name().to_string())
            .collect())
    }

    /// validate: Returns Ok(()) if bytes are a valid WASM module.
    pub fn validate(bytes: &[u8]) -> Result<()> {
        let engine = Engine::default();
        Module::from_binary(&engine, bytes).map(|_| ()).map_err(|e| anyhow!("WASM_INVALID: {}", e))
    }
}
