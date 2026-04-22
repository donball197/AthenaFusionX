use anyhow::Result;
use wasmtime::*;

pub struct WasmRuntime;

impl WasmRuntime {
    pub fn run_bytes(bytes: &[u8], func_name: &str, input: f32) -> Result<f32> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        let module = Module::from_binary(&engine, bytes)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(10_000_000)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<f32, f32>(&mut store, func_name)?;
        Ok(func.call(&mut store, input)?)
    }
}
