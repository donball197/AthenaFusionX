#!/usr/bin/env bash
set -e
cd /home/donball197/nexus_singularity-dev || exit 1
echo "🔬 Building Fair Zig and Rust WASM cells..."

# 1. Build the Zig Cell (Static Memory)
cat > /tmp/benchmark_zig.zig << 'EOF'
const std = @import("std");
var buffer = [_]u8{42} ** 8192;

export fn resonance_benchmark(iterations: u32) f32 {
    var total: f32 = 0.0;
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        var sum: f32 = 0.0;
        for (buffer) |b| {
            const v = @as(f32, @floatFromInt(b)) / 255.0;
            sum += v * v;
        }
        total += sum;
    }
    return total / @as(f32, @floatFromInt(iterations));
}
EOF
zig build-lib /tmp/benchmark_zig.zig -target wasm32-freestanding -O ReleaseFast -dynamic --name zig_bench -femit-bin=/tmp/zig_bench.wasm

# 2. Build the Rust Cell (Static Memory for fairness)
mkdir -p /tmp/rust_bench/src
cat > /tmp/rust_bench/Cargo.toml << 'EOF'
[package]
name = "rust_bench"
version = "0.1.0"
edition = "2021"
[lib]
crate-type = ["cdylib"]
[profile.release]
opt-level = 3
lto = true
panic = 'abort'
codegen-units = 1
EOF

cat > /tmp/rust_bench/src/lib.rs << 'EOF'
static mut BUFFER: [u8; 8192] = [42; 8192];

#[no_mangle]
pub extern "C" fn resonance_benchmark(iterations: u32) -> f32 {
    let mut total = 0.0f32;
    for _ in 0..iterations {
        let mut sum = 0.0f32;
        unsafe {
            for &b in &BUFFER {
                let v = b as f32 / 255.0;
                sum += v * v;
            }
        }
        total += sum;
    }
    total / iterations as f32
}
EOF
cd /tmp/rust_bench
cargo build --target wasm32-unknown-unknown --release --quiet
cp target/wasm32-unknown-unknown/release/rust_bench.wasm /tmp/rust_bench.wasm

cd /home/donball197/nexus_singularity-dev

# 3. Create the Benchmark Runner inside the Nexus Project
cat > src/bin/bench_wasm.rs << 'EOF'
use wasmtime::*;
use std::time::Instant;

fn run_wasmtime(name: &str, path: &str) {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    let module = Module::from_file(&engine, path).expect("Failed to load WASM");
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");
    let func = instance.get_typed_func::<u32, f32>(&mut store, "resonance_benchmark").unwrap();

    // Warmup (JIT compilation phase)
    let _ = func.call(&mut store, 10).unwrap();

    // Actual Benchmark
    let start = Instant::now();
    let result = func.call(&mut store, 10_000).unwrap();
    let duration = start.elapsed();

    println!("{:>18} | {:>8.2} ms | Result: {:.4}", 
        name, duration.as_secs_f64() * 1000.0, result);
}

fn main() {
    println!("\n=== NEXUS WASM RESONANCE BENCHMARK (10,000 ITERATIONS) ===\n");
    println!("Runtime/Language   | Execution Time | Validation");
    println!("--------------------------------------------------");
    run_wasmtime("Wasmtime (Zig)", "/tmp/zig_bench.wasm");
    run_wasmtime("Wasmtime (Rust)", "/tmp/rust_bench.wasm");
    println!("\n✅ Benchmark complete!");
}
EOF

# 4. Execute safely using Cargo
echo "🚀 Executing benchmark suite via Cargo..."
cargo run --release -j 1 --bin bench_wasm
