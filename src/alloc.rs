// Using a high-performance allocator for ARM64 AI processing
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub fn initialize_mnemnos_lean() {
    println!(">> [MEM] Mimalloc active. Memory architecture optimized for ARM64.");
}
