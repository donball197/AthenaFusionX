//! Proportional RoPE Kernel Specification for Gemma 4 E2B.
//! head_dim differs by attention mode -- confirmed against real weight shapes:
//!   Sliding: head_dim=256, rotary_dim=256 (full rotary), theta=1e4, rope_type=default
//!   Global:  head_dim=512, rotary_dim=128 (0.25 partial rotary), theta=1e6, rope_type=proportional
//! Frequency basis denominator uses head_dim (not rotary_dim) per Gemma's
//! "proportional" RoPE variant. No silent fallbacks -- validate() enforces
//! both known configurations explicitly.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
            // Confirmed via real layer.4 tensor shapes: q_proj[4096,1536] /
            // k_proj[512,1536] / o_proj[1536,4096] / q_norm[512] / k_norm[512]
            // -> head_dim=512 (8 heads * 512 = 4096), matching config's
            // global_head_dim. partial_rotary_factor=0.25 of 512 = 128.
            AttentionMode::Global => Self { head_dim: 512, rotary_dim: 128, theta: 1_000_000.0 },
            // Confirmed via real layer.0 tensor shapes: q_proj[2048,1536] /
            // k_proj[256,1536] / o_proj[1536,2048] / q_norm[256] / k_norm[256]
            // -> head_dim=256 (8 heads * 256 = 2048), full rotary.
            AttentionMode::Sliding => Self { head_dim: 256, rotary_dim: 256, theta: 10_000.0 },
        }
    }

    pub fn validate(&self) {
        assert!(
            self.head_dim == 256 || self.head_dim == 512,
            "FATAL: head_dim must be 256 (sliding) or 512 (global) per confirmed model spec, got {}",
            self.head_dim
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
/// Transforms Q or K vectors in place. Dimensions beyond rotary_dim act as identity.
pub fn apply_rope_scalar(vec: &mut [f32], position: usize, config: &RoPEConfig) {
    config.validate();
    assert_eq!(
        vec.len(),
        config.head_dim,
        "FATAL: Vector length mismatch with head_dim specification."
    );

    let half_rot = config.rotary_dim / 2;

    for i in 0..half_rot {
        // CRITICAL: Frequency basis denominator explicitly uses head_dim,
        // NOT rotary_dim -- this is the "proportional" RoPE variant.
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
/// # Safety
///
/// `vec` must be a valid, properly aligned slice of at least
/// `config.head_dim` elements; `position` and `config` must describe a
/// layout consistent with `vec`'s length.
pub unsafe fn apply_rope_neon(vec: &mut [f32], position: usize, config: &RoPEConfig) {
    // Phase 1 initial state: delegate to scalar reference until NEON vector layout is verified
    apply_rope_scalar(vec, position, config);
}
