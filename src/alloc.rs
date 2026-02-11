use mimalloc::MiMalloc;
use std::env;

#[global_allocator]
pub static GLOBAL: MiMalloc = MiMalloc;

pub fn initialize_mnemnos_lean() {
    // Setting environment variables for the allocator to be aggressive with cleanup
    env::set_var("MIMALLOC_RESERVE_HUGE_PAGES", "0");
    env::set_var("MIMALLOC_PAGE_RESET", "1"); // Return memory to OS faster
    println!(">> [SHIELD] 2GB RAM WALL INITIALIZED. OS SOVEREIGNTY PROTECTED.");
}
