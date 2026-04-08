use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub fn initialize_mnemnos_lean() {
    println!(">> [MEM] Mimalloc active — optimized for ARM64 low fragmentation.");
}
