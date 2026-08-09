#!/bin/bash
# forge_kv_stable.sh - Project Nexus Singularity
# Adds StaticKVCache on top of the proven loader + NEON path

set -euo pipefail

echo ">> [NEXUS] Adding stable StaticKVCache..."

# ----------------------------------------------------------------------
# 1. KV Cache (fixed capacity, single allocation)
# ----------------------------------------------------------------------
cat << 'EOF' > crates/nexus_edge_minion/src/kv_cache.rs
use half::bf16;

/// Fixed-capacity ring buffer for K/V.
/// One allocation at construction → no further heap growth.
pub struct StaticKVCache {
    k_data: Vec<bf16>,
    v_data: Vec<bf16>,
    max_seq_len: usize,
    head_dim: usize,
    cursor: usize,
    active_tokens: usize,
}

impl StaticKVCache {
    pub fn new(max_seq_len: usize, head_dim: usize) -> Self {
        let total = max_seq_len * head_dim;
        Self {
            k_data: vec![bf16::from_f32(0.0); total],
            v_data: vec![bf16::from_f32(0.0); total],
            max_seq_len,
            head_dim,
            cursor: 0,
            active_tokens: 0,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, k: &[bf16], v: &[bf16]) {
        debug_assert_eq!(k.len(), self.head_dim);
        debug_assert_eq!(v.len(), self.head_dim);

        let off = self.cursor * self.head_dim;
        self.k_data[off..off + self.head_dim].copy_from_slice(k);
        self.v_data[off..off + self.head_dim].copy_from_slice(v);

        self.cursor = (self.cursor + 1) % self.max_seq_len;
        if self.active_tokens < self.max_seq_len {
            self.active_tokens += 1;
        }
    }

    #[inline(always)]
    pub fn k_at(&self, pos: usize) -> &[bf16] {
        let off = pos * self.head_dim;
        &self.k_data[off..off + self.head_dim]
    }

    #[inline(always)]
    pub fn v_at(&self, pos: usize) -> &[bf16] {
        let off = pos * self.head_dim;
        &self.v_data[off..off + self.head_dim]
    }

    pub fn len(&self) -> usize {
        self.active_tokens
    }

    pub fn capacity(&self) -> usize {
        self.max_seq_len
    }
}
EOF

# ----------------------------------------------------------------------
# 2. Main – proven loader + NEON + KV cache write
# ----------------------------------------------------------------------
cat << 'EOF' > crates/nexus_edge_minion/src/main.rs
mod safetensors;
mod neon_math;
mod kv_cache;

use half::bf16;
use safetensors::EdgeLoader;
use kv_cache::StaticKVCache;
use std::time::Instant;

fn main() {
    let model_path = std::env::var("NEXUS_CACHE_DIR")
        .map(|d| format!("{}/model.safetensors", d))
        .unwrap_or_else(|_| {
            "/mnt/chromeos/removable/Memorex USB/AthenaFusionX/gemma-4-e2b-weights/model.safetensors"
                .to_string()
        });

    println!(">> [MINION] Zero-copy + NEON + StaticKVCache");
    let loader = EdgeLoader::new(&model_path).expect("Loader failed");

    // Proven keys
    let q_raw = loader.get_tensor("model.language_model.layers.0.self_attn.q_proj.weight").unwrap();
    let k_raw = loader.get_tensor("model.language_model.layers.0.self_attn.k_proj.weight").unwrap();
    let v_raw = loader.get_tensor("model.language_model.layers.0.self_attn.v_proj.weight").unwrap();

    let q = q_raw.as_bf16().unwrap();
    let k = k_raw.as_bf16().unwrap();
    let v = v_raw.as_bf16().unwrap();

    let hidden = q.shape[1] as usize; // 1536
    let q_dim  = q.shape[0] as usize; // 2048
    let kv_dim = k.shape[0] as usize; // 256

    // Fixed KV cache – single allocation (~1 MB)
    let mut cache = StaticKVCache::new(1024, kv_dim);
    println!("  KV cache capacity {} tokens × {} dim  (~{} KB)",
        cache.capacity(), kv_dim, (1024 * kv_dim * 4) / 1024);

    // Working set (tiny)
    let x      = vec![bf16::from_f32(0.05); hidden];
    let mut qy = vec![0.0f32; q_dim];
    let mut ky = vec![0.0f32; kv_dim];
    let mut vy = vec![0.0f32; kv_dim];
    let mut k_bf16 = vec![bf16::from_f32(0.0); kv_dim];
    let mut v_bf16 = vec![bf16::from_f32(0.0); kv_dim];

    println!(">> [MINION] NEON projections + cache write...");
    let t0 = Instant::now();

    // Q, K, V projections
    neon_math::matvec_bf16_to_f32(q.data, &x, &mut qy, q_dim, hidden);
    neon_math::matvec_bf16_to_f32(k.data, &x, &mut ky, kv_dim, hidden);
    neon_math::matvec_bf16_to_f32(v.data, &x, &mut vy, kv_dim, hidden);

    // Convert only the small K/V results (256 elems) to bf16 and store
    for i in 0..kv_dim {
        k_bf16[i] = bf16::from_f32(ky[i]);
        v_bf16[i] = bf16::from_f32(vy[i]);
    }
    cache.push(&k_bf16, &v_bf16);

    let elapsed = t0.elapsed();
    println!(">> [MINION] Done in {:?}", elapsed);
    println!("  Q[0]={:.5}", qy[0]);
    println!("  Cached K[0]={:.5}  V[0]={:.5}", 
        cache.k_at(0)[0].to_f32(), cache.v_at(0)[0].to_f32());
    println!("  Tokens in cache: {}", cache.len());
    println!(">> [MINION] Full pipeline stable.");
}
EOF

echo ""
echo ">> [NEXUS] StaticKVCache added."
echo "Run with:"
echo "  export LD_LIBRARY_PATH=\"\$(pwd)/lib:\$LD_LIBRARY_PATH\""
echo "  cargo run -p nexus_edge_minion --release"
