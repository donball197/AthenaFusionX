#!/bin/bash
# forge_decoder_layer.sh - corrected rope.rs, attention.rs, and new decoder_layer.rs
# for the Gemma 4 E2B text-generation path in nexus_edge_minion.
#
# Fixes/additions vs. prior state:
#   - rope.rs: Global mode is head_dim=512 / rotary_dim=128 (was wrongly 256/64),
#     confirmed against real q_proj/k_proj/o_proj/q_norm/k_norm tensor shapes.
#     validate() now accepts 256 (sliding) or 512 (global) instead of hard-asserting 256.
#   - attention.rs: rewritten to do REAL q_proj/k_proj/v_proj matvecs via
#     neon_math::matvec_bf16_to_f32, per-head q_norm/k_norm RMSNorm (Gemma's
#     (1+weight) convention) applied BEFORE RoPE, and pushes K/V into the
#     StaticKVCache. Does NOT yet compute attention scores (QK^T/softmax/*V) --
#     that's still the next roadmap milestone, not silently included here.
#   - decoder_layer.rs (new): the full 4-norm Gemma2-style sandwich + MLP +
#     PLE stage 2 injection + layer_scalar, wired with real weight shapes.
#     Attention contribution is a placeholder zero vector until QK^T/softmax
#     exists (see TODO in the file) -- everything else is real.
#
# Known gaps NOT addressed by this script (flagging rather than silently
# omitting):
#   - StaticKVCache must be constructed per-layer-type (head_dim differs:
#     256 sliding / 512 global) -- one shared cache sized for one head_dim
#     will not work for both layer types.
#   - num_kv_shared_layers=20: layers 15-34 should reuse an earlier layer's
#     K/V instead of computing their own -- decoder_layer.rs always computes
#     its own K/V right now. Wiring shared-KV routing is not in this pass.
#   - layer_scalar's exact application point (whole hidden_state vs. one
#     residual branch) is applied here as a whole-hidden_state multiply at
#     the very end of the layer -- confirmed real, but this specific
#     application point is a best guess, not verified against primary source.

set -euo pipefail

echo ">> [NEXUS] Writing corrected lib/nexus_loader dependents (Rust side)..."
mkdir -p crates/nexus_edge_minion/src

# ==============================================================================
# 1. rope.rs (corrected head_dim/rotary_dim for Global mode)
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/rope.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/rope.rs
//! Proportional RoPE Kernel Specification for Gemma 4 E2B.
//! head_dim differs by attention mode -- confirmed against real weight shapes:
//!   Sliding: head_dim=256, rotary_dim=256 (full rotary), theta=1e4, rope_type=default
//!   Global:  head_dim=512, rotary_dim=128 (0.25 partial rotary), theta=1e6, rope_type=proportional
//! Frequency basis denominator uses head_dim (not rotary_dim) per Gemma's
//! "proportional" RoPE variant. No silent fallbacks -- validate() enforces
//! both known configurations explicitly.

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
            // Confirmed via real layer.4 tensor shapes: q_proj[4096,1536] /
            // k_proj[512,1536] / o_proj[1536,4096] / q_norm[512] / k_norm[512]
            // -> head_dim=512 (8 heads * 512 = 4096), matching config's
            // global_head_dim. partial_rotary_factor=0.25 of 512 = 128.
            AttentionMode::Global => Self {
                head_dim: 512,
                rotary_dim: 128,
                theta: 1_000_000.0,
            },
            // Confirmed via real layer.0 tensor shapes: q_proj[2048,1536] /
            // k_proj[256,1536] / o_proj[1536,2048] / q_norm[256] / k_norm[256]
            // -> head_dim=256 (8 heads * 256 = 2048), full rotary.
            AttentionMode::Sliding => Self {
                head_dim: 256,
                rotary_dim: 256,
                theta: 10_000.0,
            },
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
pub unsafe fn apply_rope_neon(
    vec: &mut [f32],
    position: usize,
    config: &RoPEConfig,
) {
    // Phase 1 initial state: delegate to scalar reference until NEON vector layout is verified
    apply_rope_scalar(vec, position, config);
}
EOF

# ==============================================================================
# 2. attention.rs (real q/k/v projection + qk_norm + RoPE + KV cache push)
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/attention.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/attention.rs
//! Self-attention projection + norm + RoPE + KV-cache pipeline for one layer.
//!
//! NOTE: this produces per-head Q (RoPE'd, ready to score) and pushes K/V for
//! this position into the layer's StaticKVCache -- it does NOT compute
//! attention scores (QK^T / softmax / *V). That's still the next roadmap
//! milestone; the returned Q is what that step will consume.

use crate::rope::{apply_rope_scalar, RoPEConfig, AttentionMode};
use crate::kv_cache::StaticKVCache;
use crate::neon_math::matvec_bf16_to_f32;
use half::bf16;

/// RMSNorm applied in place using Gemma's (1 + weight) convention -- Gemma's
/// RMSNorm weights are zero-initialized and used as a delta from 1.0, unlike
/// Llama-style RMSNorm which uses the raw weight. Getting this wrong is a
/// well-documented silent-garbage bug across multiple independent Gemma ports.
pub fn rms_norm_in_place(x: &mut [f32], weight: &[f32]) {
    debug_assert_eq!(x.len(), weight.len());
    const EPS: f32 = 1e-6;
    let mean_sq: f32 = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
    let scale = 1.0 / (mean_sq + EPS).sqrt();
    for (xi, wi) in x.iter_mut().zip(weight.iter()) {
        *xi = *xi * scale * (1.0 + wi);
    }
}

pub struct AttentionBlock {
    pub layer_idx: usize,
    pub mode: AttentionMode,
    pub rope_config: RoPEConfig,
    pub num_heads: usize,
    pub num_kv_heads: usize, // MQA: 1 for this checkpoint
    pub hidden_size: usize,  // 1536
}

impl AttentionBlock {
    pub fn new(layer_idx: usize, is_global: bool) -> Self {
        let mode = if is_global { AttentionMode::Global } else { AttentionMode::Sliding };
        Self {
            layer_idx,
            mode,
            rope_config: RoPEConfig::new(mode),
            num_heads: 8,
            num_kv_heads: 1,
            hidden_size: 1536,
        }
    }

    /// Projects the already-normalized hidden state through q_proj/k_proj/v_proj,
    /// applies per-head q_norm/k_norm then RoPE to Q and K, and pushes this
    /// position's K/V into the cache. Returns Q ([num_heads*head_dim]), RoPE'd
    /// and ready for attention scoring once that exists.
    ///
    /// Weight shapes expected (real, confirmed from model.safetensors):
    ///   q_proj: [num_heads*head_dim, hidden_size]
    ///   k_proj, v_proj: [num_kv_heads*head_dim, hidden_size]
    ///   q_norm, k_norm: [head_dim] (shared across heads)
    #[allow(clippy::too_many_arguments)]
    pub fn project_and_cache(
        &self,
        hidden_state_normed: &[f32],
        q_proj_w: &[bf16],
        k_proj_w: &[bf16],
        v_proj_w: &[bf16],
        q_norm_w: &[f32],
        k_norm_w: &[f32],
        position: usize,
        kv_cache: &mut StaticKVCache,
    ) -> Vec<f32> {
        let head_dim = self.rope_config.head_dim;
        let q_out_dim = self.num_heads * head_dim;
        let kv_out_dim = self.num_kv_heads * head_dim;

        assert_eq!(hidden_state_normed.len(), self.hidden_size);
        assert_eq!(q_proj_w.len(), q_out_dim * self.hidden_size);
        assert_eq!(k_proj_w.len(), kv_out_dim * self.hidden_size);
        assert_eq!(v_proj_w.len(), kv_out_dim * self.hidden_size);
        assert_eq!(q_norm_w.len(), head_dim);
        assert_eq!(k_norm_w.len(), head_dim);

        // Activations quantized to bf16 to match weight precision for the NEON
        // matvec kernel -- deliberate bf16-throughout design choice.
        let x_bf16: Vec<bf16> = hidden_state_normed.iter().map(|&v| bf16::from_f32(v)).collect();

        let mut q = vec![0.0f32; q_out_dim];
        let mut k = vec![0.0f32; kv_out_dim];
        let mut v = vec![0.0f32; kv_out_dim];

        matvec_bf16_to_f32(q_proj_w, &x_bf16, &mut q, q_out_dim, self.hidden_size);
        matvec_bf16_to_f32(k_proj_w, &x_bf16, &mut k, kv_out_dim, self.hidden_size);
        matvec_bf16_to_f32(v_proj_w, &x_bf16, &mut v, kv_out_dim, self.hidden_size);

        // MQA: single KV head (num_kv_heads == 1 for this checkpoint) -- norm
        // and RoPE applied once. V bypasses norm and RoPE entirely.
        rms_norm_in_place(&mut k[..head_dim], k_norm_w);
        apply_rope_scalar(&mut k[..head_dim], position, &self.rope_config);

        let k_bf16: Vec<bf16> = k.iter().map(|&x| bf16::from_f32(x)).collect();
        let v_bf16: Vec<bf16> = v.iter().map(|&x| bf16::from_f32(x)).collect();
        kv_cache.push(&k_bf16[..head_dim], &v_bf16[..head_dim]);

        // Per-head Q: norm + RoPE applied independently to each of num_heads slices.
        for h in 0..self.num_heads {
            let start = h * head_dim;
            let end = start + head_dim;
            rms_norm_in_place(&mut q[start..end], q_norm_w);
            apply_rope_scalar(&mut q[start..end], position, &self.rope_config);
        }

        q
    }
}
EOF

# ==============================================================================
# 3. decoder_layer.rs (new: full 4-norm sandwich + MLP + PLE stage 2 + layer_scalar)
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/decoder_layer.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/decoder_layer.rs
//! Full Gemma4DecoderLayer sandwich: 4 RMSNorms, self-attention setup, MLP,
//! PLE stage 2 injection, layer_scalar.
//!
//! NOTE: the actual QK^T / softmax / *V attention score computation is not
//! yet implemented -- that's the next roadmap milestone. Until it exists,
//! the attention branch's contribution is a placeholder zero vector, which
//! makes this layer act as identity-on-attention (residual passes through
//! unchanged) while MLP + PLE + layer_scalar are fully real and exercisable
//! now. Generation will not be coherent until attention scoring is wired in.

use half::bf16;
use crate::attention::{AttentionBlock, rms_norm_in_place};
use crate::kv_cache::StaticKVCache;
use crate::neon_math::matvec_bf16_to_f32;

const HIDDEN_SIZE: usize = 1536;
const PER_LAYER_DIM: usize = 256;

/// Borrowed per-layer weights for one forward call. Loading/caching these
/// from EdgeLoader is inference_engine's job, not this struct's.
pub struct DecoderLayerWeights<'a> {
    pub input_layernorm: &'a [f32],
    pub post_attention_layernorm: &'a [f32],
    pub pre_feedforward_layernorm: &'a [f32],
    pub post_feedforward_layernorm: &'a [f32],

    pub q_proj: &'a [bf16],
    pub k_proj: &'a [bf16],
    pub v_proj: &'a [bf16],
    pub o_proj: &'a [bf16],
    pub q_norm: &'a [f32],
    pub k_norm: &'a [f32],

    pub gate_proj: &'a [bf16],
    pub up_proj: &'a [bf16],
    pub down_proj: &'a [bf16],
    pub mlp_intermediate_size: usize, // 6144

    pub per_layer_input_gate: &'a [bf16],
    pub per_layer_projection: &'a [bf16],
    pub post_per_layer_input_norm: &'a [f32],

    // Confirmed real (HF transformers issue #45482) but exact application
    // point unverified against primary source -- applied here as a
    // whole-hidden_state multiply at the very end of the layer. Revisit if
    // generation output looks systematically scaled wrong.
    pub layer_scalar: f32,
}

pub struct DecoderLayer {
    pub attn: AttentionBlock,
}

impl DecoderLayer {
    pub fn new(layer_idx: usize, is_global: bool) -> Self {
        Self { attn: AttentionBlock::new(layer_idx, is_global) }
    }

    /// Runs one token through the full layer sandwich. `hidden_state` is
    /// [HIDDEN_SIZE] and updated in place. `layer_ple_slice` is this layer's
    /// 256-dim slice of the Stage-1 per_layer_inputs tensor (computed once
    /// per token, before the layer loop, by Stage1PLE).
    pub fn forward(
        &self,
        hidden_state: &mut [f32],
        layer_ple_slice: &[f32],
        weights: &DecoderLayerWeights,
        position: usize,
        kv_cache: &mut StaticKVCache,
    ) {
        assert_eq!(hidden_state.len(), HIDDEN_SIZE);
        assert_eq!(layer_ple_slice.len(), PER_LAYER_DIM);

        // ---- Attention block ----
        let residual = hidden_state.to_vec();
        let mut x = hidden_state.to_vec();
        rms_norm_in_place(&mut x, weights.input_layernorm);

        let _q = self.attn.project_and_cache(
            &x,
            weights.q_proj,
            weights.k_proj,
            weights.v_proj,
            weights.q_norm,
            weights.k_norm,
            position,
            kv_cache,
        );

        // TODO(roadmap item 2): compute_attention_output(&_q, kv_cache) ->
        // [num_heads*head_dim], scored via softmax(QK^T/sqrt(head_dim))*V,
        // then o_proj it back down to HIDDEN_SIZE. Until that exists this
        // branch is a placeholder zero vector so the rest of the sandwich
        // (norms, residual wiring, MLP, PLE) can be exercised independently.
        let mut attn_out = vec![0.0f32; HIDDEN_SIZE];

        rms_norm_in_place(&mut attn_out, weights.post_attention_layernorm);
        for i in 0..HIDDEN_SIZE {
            hidden_state[i] = residual[i] + attn_out[i];
        }

        // ---- MLP block ----
        let residual = hidden_state.to_vec();
        let mut x = hidden_state.to_vec();
        rms_norm_in_place(&mut x, weights.pre_feedforward_layernorm);

        let inter = weights.mlp_intermediate_size;
        let x_bf16: Vec<bf16> = x.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut gate = vec![0.0f32; inter];
        let mut up = vec![0.0f32; inter];
        matvec_bf16_to_f32(weights.gate_proj, &x_bf16, &mut gate, inter, HIDDEN_SIZE);
        matvec_bf16_to_f32(weights.up_proj, &x_bf16, &mut up, inter, HIDDEN_SIZE);

        // gelu_pytorch_tanh per config's hidden_activation, gated (SwiGLU-shaped, GELU variant)
        let mut gated = vec![0.0f32; inter];
        for i in 0..inter {
            gated[i] = gelu_tanh(gate[i]) * up[i];
        }
        let gated_bf16: Vec<bf16> = gated.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut mlp_out = vec![0.0f32; HIDDEN_SIZE];
        matvec_bf16_to_f32(weights.down_proj, &gated_bf16, &mut mlp_out, HIDDEN_SIZE, inter);

        rms_norm_in_place(&mut mlp_out, weights.post_feedforward_layernorm);
        for i in 0..HIDDEN_SIZE {
            hidden_state[i] = residual[i] + mlp_out[i];
        }

        // ---- PLE stage 2 injection ----
        // gate = act_fn(per_layer_input_gate(hidden_states))  [1536->256]
        let x_bf16: Vec<bf16> = hidden_state.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut ple_gate = vec![0.0f32; PER_LAYER_DIM];
        matvec_bf16_to_f32(weights.per_layer_input_gate, &x_bf16, &mut ple_gate, PER_LAYER_DIM, HIDDEN_SIZE);
        for g in ple_gate.iter_mut() {
            *g = gelu_tanh(*g);
        }

        // injected = gate * per_layer_inputs[layer_idx]
        let mut injected = vec![0.0f32; PER_LAYER_DIM];
        for i in 0..PER_LAYER_DIM {
            injected[i] = ple_gate[i] * layer_ple_slice[i];
        }

        // projected = per_layer_projection(injected)  [256->1536]
        let injected_bf16: Vec<bf16> = injected.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut ple_out = vec![0.0f32; HIDDEN_SIZE];
        matvec_bf16_to_f32(weights.per_layer_projection, &injected_bf16, &mut ple_out, HIDDEN_SIZE, PER_LAYER_DIM);

        // normed = post_per_layer_input_norm(projected); hidden_states += normed
        rms_norm_in_place(&mut ple_out, weights.post_per_layer_input_norm);
        for i in 0..HIDDEN_SIZE {
            hidden_state[i] += ple_out[i];
        }

        // ---- layer_scalar (confirmed real; exact application point unverified) ----
        for v in hidden_state.iter_mut() {
            *v *= weights.layer_scalar;
        }
    }
}

fn gelu_tanh(x: f32) -> f32 {
    let inner = 0.7978845608_f32 * (x + 0.044715_f32 * x.powi(3));
    0.5 * x * (1.0 + inner.tanh())
}
EOF

# ==============================================================================
# 4. lib.rs: add decoder_layer module (rest of module list assumed already fixed)
# ==============================================================================
echo ">> [NEXUS] Ensuring lib.rs declares decoder_layer..."
LIB_RS="crates/nexus_edge_minion/src/lib.rs"
if [ -f "$LIB_RS" ] && ! grep -q "pub mod decoder_layer;" "$LIB_RS"; then
    echo "pub mod decoder_layer;" >> "$LIB_RS"
    echo ">> [NEXUS] Appended 'pub mod decoder_layer;' to lib.rs"
else
    echo ">> [NEXUS] lib.rs either missing or already declares decoder_layer -- check manually."
fi

echo ""
echo ">> [NEXUS] Done. Rebuild with:"
echo "     cargo build --features local_edge"
echo ""
echo ">> [NEXUS] Reminder of what's still open after this pass:"
echo "     - StaticKVCache must be constructed per layer-type (head_dim 256 vs 512)"
echo "     - num_kv_shared_layers=20 (layers 15-34) not yet routed to reuse earlier K/V"
echo "     - attention scoring (QK^T/softmax/*V) not implemented -- attn branch is a placeholder"
echo ""
