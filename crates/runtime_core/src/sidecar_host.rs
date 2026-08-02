use anyhow::Result;
use std::path::Path;
use wasmtime::{Caller, Engine, Linker, Module, Store};
use wasmtime_wasi::p1::{self, WasiP1Ctx};
use wasmtime_wasi::WasiCtxBuilder;

// 1. Define a custom state that holds BOTH WASI and your own host resources
pub struct HostState {
    pub wasi: WasiP1Ctx,
    // Add your turbovec index or other host resources here later:
    // pub vector_vault: Option<std::sync::Arc<VectorVault>>,
}

pub struct SidecarHost {
    engine: Engine,
}

impl SidecarHost {
    pub fn new(_dummy: bool) -> Result<Self> {
        let mut config = wasmtime::Config::new();
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        let engine = Engine::new(&config)?;
        Ok(Self { engine })
    }

    pub fn load_and_run(&self, wasm_path: &Path) -> Result<()> {
        let module = Module::from_file(&self.engine, wasm_path)?;

        // 2. Linker must now be typed to our custom HostState
        let mut linker: Linker<HostState> = Linker::new(&self.engine);

        // 3. Link WASI by providing a closure to extract WasiP1Ctx from HostState
        p1::add_to_linker_sync(&mut linker, |state: &mut HostState| &mut state.wasi)?;

        // =====================================================================
        // ⚡ HOST FUNCTION BRIDGE: This is where we expose Rust to Wasm
        // =====================================================================
        linker.func_wrap(
            "env",
            "search_memories",
            |_caller: Caller<'_, HostState>, ptr: u32, dim: u32| -> u32 {
                println!(
                    ">> [HOST] Intercepted Wasm call to search_memories(ptr: {}, dim: {})",
                    ptr, dim
                );

                // Memory bridge logic goes here...
                // 1. Read float array from caller.get_export("memory")
                // 2. Pass to turbovec
                // 3. Write results back

                0 // Return memory pointer of results
            },
        )?;
        // =====================================================================

        let wasi = WasiCtxBuilder::new().inherit_stdio().build_p1();

        // 4. Initialize the Store with our composite HostState
        let mut store = Store::new(&self.engine, HostState { wasi });

        let instance = linker.instantiate(&mut store, &module)?;

        let start = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
        start.call(&mut store, ())?;

        Ok(())
    }
}
