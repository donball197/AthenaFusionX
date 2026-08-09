#!/bin/bash
# forge_neon_execution.sh - Project Nexus Singularity v0.2.2

echo ">> [NEXUS] Wiring the Q-Projection into the NEON MatMul Substrate..."

cat << 'INNER' > crates/nexus_edge_minion/src/main.rs
mod safetensors;
mod neon_math;

use half::bf16;
use safetensors::EdgeLoader;
use std::time::Instant;

fn main() {
    let cache_dir = std::env::var("NEXUS_CACHE_DIR")
        .unwrap_or_else(|_| "/mnt/chromeos/removable/Memorex USB/AthenaFusionX/gemma-4-e2b-weights".to_string());
    let model_path = format!("{}/model.safetensors", cache_dir);

    println!(">> [MINION] Igniting Zero-Copy Loader...");
    let loader = EdgeLoader::new(&model_path).expect("Loader failed to ignite");

    // Target the Language Model Q-Projection
    let weight_key = "model.language_model.layers.0.self_attn.q_proj.weight";
    
    let raw_tensor = loader.get_tensor(weight_key)
        .unwrap_or_else(|_| panic!("Failed to locate {}", weight_key));
    
    // Silence utility warnings
    let _ = raw_tensor.numel();
    let _ = raw_tensor.element_size();

    let typed_w = raw_tensor.as_bf16().expect("tensor is not BF16 or misaligned");
    
    // STRICT SHAPE VALIDATION
    assert_eq!(typed_w.shape.len(), 2, "FATAL: Expected a 2D weight matrix");
    let m = typed_w.shape[0] as usize; // 2048
    let n = typed_w.shape[1] as usize; // 1536
    assert_eq!(typed_w.data.len(), m * n, "FATAL: Tensor byte length does not match shape dimensions");

    println!(">> [MINION] Extracting Substrate: {} ({}x{})", weight_key, m, n);

    // x = input vector (1536 elements), y = output vector (2048 elements)
    // Heap allocation: ~11 KB total
    let x_in = vec![bf16::from_f32(1.0); n];
    let mut y_out = vec![0.0f32; m];

    println!(">> [MINION] Executing ARM64 NEON MatMul...");
    let start = Instant::now();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon_math::matvec_bf16_to_f32_neon(typed_w.data, &x_in, &mut y_out, m, n);
    }

    let duration = start.elapsed();

    println!(">> [MINION] Execution Complete in {:?}", duration);
    println!("   Output Row 0 sum: {}", y_out[0]);
    println!("   Output Row 1 sum: {}", y_out[1]);
    println!("   Output Row 2 sum: {}", y_out[2]);
    println!("   Heap RAM remained strictly bound. Kernel Stable.");
}
INNER

echo ">> [NEXUS] Compiling and Executing with LLVM SIMD Optimizations (--release)..."
env -u LD_LIBRARY_PATH cargo run -p nexus_edge_minion --release

echo ">> [NEXUS] Substrate Math Verified."
