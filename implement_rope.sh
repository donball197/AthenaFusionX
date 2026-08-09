#!/bin/bash
data/set -euo pipefail

echo ">> [PHASE 1] Creating strict Proportional RoPE module in nexus_edge_minion..."

mkdir -p crates/nexus_edge_minion/src

cat << 'INNER' > crates/nexus_edge_minion/src/rope.rs
//! Proportional RoPE Kernel Specification for Gemma 4 E2B
//! Strict enforcement: head_dim = 256, rotary_dim = 64 (global) or 256 (sliding).
//! Frequency basis explicitly computed using head_dim = 256. No silent fallbacks.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionMode {
    Global,
    Sliding,
}

#[derive(Debug, Clone, Copy)]
pub struct RoPEConfig {
    pub head_dim: usize,
    pub rotary_dim: usize,
    pub theta: f32,
}

impl RoPEConfig {
    pub fn new(mode: AttentionMode) -> Self {
        match mode {
            AttentionMode::Global => Self {
                head_dim: 256,
                rotary_dim: 64, // 0.25 partial rotary factor
                theta: 1_000_000.0,
            },
            AttentionMode::Sliding => Self {
                head_dim: 256,
                rotary_dim: 256, // Full default RoPE
                theta: 10_000.0,
            },
        }
    }

    pub fn validate(&self) {
        assert_eq!(
            self.head_dim, 256,
            "FATAL: Head dimension must be exactly 256 per model spec."
        );
        assert!(
            self.rotary_dim <= self.head_dim,
            "FATAL: Rotary dimension cannot exceed head dimension."
        );
        assert!(
            self.theta == 1_000_000.0 || self.theta == 10_000.0,
            "FATAL: Invalid theta configuration. Must be explicitly 1e6 or 1e4."
        );
    }
}

/// Authoritative scalar reference implementation for Proportional RoPE.
/// Transforms Q or K vectors in place. Unchanged dimensions act as identity.
pub fn apply_rope_scalar(
    vec: &mut [f32],
    position: usize,
    config: &RoPEConfig,
) {
    config.validate();
    assert_eq!(
        vec.len(),
        config.head_dim,
        "FATAL: Vector length mismatch with head_dim specification."
    );

    let half_rot = config.rotary_dim / 2;

    for i in 0..half_rot {
        // CRITICAL: Frequency basis denominator explicitly uses head_dim (256), NOT rotary_dim
        let exponent = (2 * i) as f32 / config.head_dim as f32;
        let inv_freq = 1.0 / config.theta.powf(exponent);
        let freq = position as f32 * inv_freq;
        
        let cos_val = freq.cos();
        let sin_val = freq.sin();

        let x1 = vec[i];
        let x2 = vec[i + half_rot];

        // NeoX-style split-half rotation pairing
        vec[i] = x1 * cos_val - x2 * sin_val;
        vec[i + half_rot] = x1 * sin_val + x2 * cos_val;
    }
    // Dimensions from rotary_dim .. head_dim remain untouched (identity rotation).
}

#[cfg(target_arch = "aarch64")]
pub unsafe fn apply_rope_neon(
    vec: &mut [f32],
    position: usize,
    config: &RoPEConfig,
) {
    // Phase 1 initial state: delegate to scalar reference until NEON vector layout is verified
    apply_rope_scalar(vec, position, config);
}
INNER

echo ">> [PHASE 1] Registering rope module in nexus_edge_minion lib.rs/main.rs..."
if ! grep -q "pub mod rope;" crates/nexus_edge_minion/src/lib.rs 2>/dev/null; then
    echo "pub mod rope;" >> crates/nexus_edge_minion/src/lib.rs
fi

cargo check -p nexus_edge_minion
