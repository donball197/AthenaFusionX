use std::sync::Arc;
use tokio::sync::Mutex;
use nexus_runtime_core::{
    agents::agentic_runtime::{AgenticRuntime, SubstrateContext},
    shared_state::VmoState,
    distributed_scheduler::DistributedScheduler,
    cluster::ClusterMesh,
};
use nexus_substrate::memory::MemoryPalace;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("\x1b[32m[NEXUS HEARTBEAT] Initiating System Check...\x1b[0m");

    // 1. Test Substrate (Zig Bridge)
    let handle = unsafe { nexus_substrate::kernel::ffi::zx_vmo_create_native(1024) };
    if handle > 0 {
        println!("\x1b[34m[OK]\x1b[0m Zig Substrate: VMO Created (Handle: 0x{:x})", handle);
        unsafe { nexus_substrate::kernel::ffi::zx_handle_close_native(handle) };
        println!("\x1b[34m[OK]\x1b[0m Zig Substrate: VMO Released");
    } else {
        panic!("Zig Bridge Failure: Could not create VMO");
    }

    // 2. Test Agentic Runtime (LRL)
    let memory = Arc::new(Mutex::new(MemoryPalace::new("heartbeat.db").await?));
    let vmo_state = Arc::new(Mutex::new(VmoState::default()));
    let mesh = Arc::new(ClusterMesh::new());
    let dist = Arc::new(DistributedScheduler::new(mesh));

    let mut lrl = AgenticRuntime::new(SubstrateContext {
        memory,
        vmo_state,
        dist,
    });

    println!("\x1b[34m[OK]\x1b[0m Agentic Runtime: Context Initialized");
    
    let result = lrl.dispatch("echo 'NEXUS_ALIVE'").await?;
    if result.contains("NEXUS_ALIVE") {
        println!("\x1b[34m[OK]\x1b[0m LRL Dispatcher: Shell Routing Verified");
    }

    println!("\x1b[32m[SUCCESS] NEXUS SINGULARITY V3 IS OPERATIONAL\x1b[0m");
    Ok(())
}
