use anyhow::Result;
use wasmtime::*;

pub struct WasmExecutor;

impl WasmExecutor {
    pub fn execute(wasm: &[u8], fn_name: &str) -> Result<()> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        
        let module = Module::from_binary(&engine, wasm)?;
        let mut store = Store::new(&engine, ());
        store.set_fuel(50_000)?;

        let instance = Instance::new(&mut store, &module, &[])?;
        let func = instance.get_typed_func::<(), ()>(&mut store, fn_name)?;
        func.call(&mut store, ())?;

        Ok(())
    }
}
