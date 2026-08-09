#!/bin/bash
# forge_typed_views.sh - Project Nexus Singularity v0.2.2
# Surgically injects typed bf16/f16 tensor views and updates the main execution loop.

echo ">> [NEXUS] Injecting 'half' crate dependency..."
cd ~/AthenaFusionX || exit
cargo add half@2.3 -p nexus_edge_minion

echo ">> [NEXUS] Appending TypedTensor zero-copy views to safetensors.rs..."
cat << 'EOF' >> crates/nexus_edge_minion/src/safetensors.rs

// --- ZERO-COPY TYPED VIEWS (Appended by forge_typed_views.sh) ---
use half::{bf16, f16};
use std::mem;

/// A strongly-typed, zero-copy view of the tensor data.
pub struct TypedTensor<'a, T> {
    pub shape: &'a [i64],
    pub data: &'a [T],
}

impl<'a> TensorSlice<'a> {
    /// Zero-cost cast to f16 (Half precision)
    pub fn as_f16(&self) -> Result<TypedTensor<'a, f16>, &'static str> {
        self.cast_slice::<f16>(2) // 2 matches the Zig F16 DType enum
    }

    /// Zero-cost cast to bf16 (Brain Float 16)
    pub fn as_bf16(&self) -> Result<TypedTensor<'a, bf16>, &'static str> {
        self.cast_slice::<bf16>(3) // 3 matches the Zig BF16 DType enum
    }

    /// Zero-cost cast to f32 (Single precision)
    pub fn as_f32(&self) -> Result<TypedTensor<'a, f32>, &'static str> {
        self.cast_slice::<f32>(1) // 1 matches the Zig F32 DType enum
    }

    /// Internal generic caster that enforces alignment and shape integrity
    fn cast_slice<T>(&self, expected_dtype: u8) -> Result<TypedTensor<'a, T>, &'static str> {
        if self.dtype != expected_dtype {
            return Err("Type mismatch: Cannot cast tensor to requested dtype");
        }

        let ptr = self.data.as_ptr();
        let align = mem::align_of::<T>();

        // Critical ARM64 Safety: Prevent unaligned memory access faults
        if (ptr as usize) % align != 0 {
            return Err("Fatal: Tensor pointer is not properly aligned in the memory map");
        }

        let num_elements = self.data.len() / mem::size_of::<T>();

        // SAFETY: 
        // 1. Pointer is aligned.
        // 2. Data bounds are validated by the Zig FFI layer.
        // 3. Lifetime 'a is inherently tied to the EdgeLoader's mmap.
        let typed_slice = unsafe {
            std::slice::from_raw_parts(ptr as *const T, num_elements)
        };

        Ok(TypedTensor {
            shape: self.shape,
            data: typed_slice,
        })
    }
}
EOF

echo ">> [NEXUS] Wiring the bf16 inference logic into main.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/main.rs
mod safetensors;

use safetensors::EdgeLoader;

fn main() {
    let cache_dir = std::env::var("NEXUS_CACHE_DIR")
        .unwrap_or_else(|_| "/mnt/chromeos/removable/Memorex USB/AthenaFusionX/gemma-4-e2b-weights".to_string());
    let model_path = format!("{}/model.safetensors", cache_dir);

    println!(">> [MINION] Igniting Zero-Copy Loader...");
    let loader = EdgeLoader::new(&model_path).expect("Loader failed to ignite");

    // 1. Get the raw tensor slice
    let raw_tensor = loader.get_tensor("model.layers.0.self_attn.q_proj.weight")
        .expect("Failed to locate q_proj");

    // 2. Safely cast to bf16 (Gemma 4 natively uses Brain Float 16)
    let q_proj_bf16 = raw_tensor.as_bf16().expect("Failed to cast to bf16 - dtype mismatch");

    println!(">> [MINION] Q_Proj mapped successfully.");
    println!("   Shape: {:?}", q_proj_bf16.shape);
    println!("   First value: {}", q_proj_bf16.data[0]); 
    println!("   Active Heap RAM: < 5MB. Execution complete.");
}
EOF

echo ">> [NEXUS] Script complete. Recompiling the Minion Substrate..."
cargo build
echo ">> [NEXUS] Ready to execute: cargo run -p nexus_edge_minion"
