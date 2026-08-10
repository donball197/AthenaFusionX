//! Self-attention projection + norm + RoPE + KV-cache pipeline for one layer.
//!
//! NOTE: this produces per-head Q (RoPE'd, ready to score) and pushes K/V for
//! this position into the layer's StaticKVCache -- it does NOT compute
//! attention scores (QK^T / softmax / *V). That's still the next roadmap
//! milestone; the returned Q is what that step will consume.

use crate::kv_cache::StaticKVCache;
use crate::neon_math::matvec_bf16_to_f32;
use crate::rope::{apply_rope_scalar, AttentionMode, RoPEConfig};
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
    /// For KV-shared layers: projects Q only (q_proj + q_norm + RoPE). Does
    /// NOT compute or push K/V -- this layer reuses an earlier layer's K/V
    /// (the "KV source" layer of the same attention type, per
    /// num_kv_shared_layers routing). Caller is responsible for pointing
    /// attention scoring at the correct source layer's StaticKVCache.
    pub fn project_q_only(
        &self,
        hidden_state_normed: &[f32],
        q_proj_w: &[bf16],
        q_norm_w: &[f32],
        position: usize,
    ) -> Vec<f32> {
        let head_dim = self.rope_config.head_dim;
        let q_out_dim = self.num_heads * head_dim;

        assert_eq!(hidden_state_normed.len(), self.hidden_size);
        assert_eq!(q_proj_w.len(), q_out_dim * self.hidden_size);
        assert_eq!(q_norm_w.len(), head_dim);

        let x_bf16: Vec<bf16> = hidden_state_normed.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut q = vec![0.0f32; q_out_dim];
        matvec_bf16_to_f32(q_proj_w, &x_bf16, &mut q, q_out_dim, self.hidden_size);

        for h in 0..self.num_heads {
            let start = h * head_dim;
            let end = start + head_dim;
            rms_norm_in_place(&mut q[start..end], q_norm_w);
            apply_rope_scalar(&mut q[start..end], position, &self.rope_config);
        }

        q
    }
    /// Computes scaled dot-product attention (MQA) over the StaticKVCache for all Q heads,
    /// applies softmax over sequence positions, and projects aggregated head outputs
    /// back to hidden_size (1536) via o_proj using NEON matvec acceleration.
    pub fn compute_attention_output(
        &self,
        q: &[f32],
        o_proj_w: &[bf16],
        kv_cache: &StaticKVCache,
    ) -> Vec<f32> {
        let head_dim = self.rope_config.head_dim;
        let q_out_dim = self.num_heads * head_dim; // 2048 (sliding) or 4096 (global)
        let seq_len = kv_cache.len();

        assert_eq!(q.len(), q_out_dim);
        assert_eq!(o_proj_w.len(), self.hidden_size * q_out_dim);

        if seq_len == 0 {
            return vec![0.0f32; self.hidden_size];
        }

        let scale = 1.0f32 / (head_dim as f32).sqrt();
        let mut combined_heads = vec![0.0f32; q_out_dim];

        // MQA: 8 Q heads attend to the same single K/V head in cache
        for h in 0..self.num_heads {
            let q_start = h * head_dim;
            let q_h = &q[q_start..q_start + head_dim];

            let mut scores = vec![0.0f32; seq_len];
            let mut max_score = f32::NEG_INFINITY;

            // 1. Scaled dot product Q_h . K_pos
            for pos in 0..seq_len {
                let k_bf16 = kv_cache.k_at(pos);
                let mut dot = 0.0f32;
                for i in 0..head_dim {
                    dot += q_h[i] * k_bf16[i].to_f32();
                }
                let score = dot * scale;
                scores[pos] = score;
                if score > max_score {
                    max_score = score;
                }
            }

            // 2. Numerically stable Softmax
            let mut exp_sum = 0.0f32;
            for pos in 0..seq_len {
                let exp_val = (scores[pos] - max_score).exp();
                scores[pos] = exp_val;
                exp_sum += exp_val;
            }

            let inv_exp_sum = if exp_sum > 0.0 { 1.0 / exp_sum } else { 0.0 };
            for pos in 0..seq_len {
                scores[pos] *= inv_exp_sum;
            }

            // 3. Weighted aggregation of V_pos
            let out_start = h * head_dim;
            let head_out = &mut combined_heads[out_start..out_start + head_dim];

            for pos in 0..seq_len {
                let v_bf16 = kv_cache.v_at(pos);
                let weight = scores[pos];
                for i in 0..head_dim {
                    head_out[i] += weight * v_bf16[i].to_f32();
                }
            }
        }

        // 4. Project combined head representations back to hidden_size (1536) via o_proj
        let combined_bf16: Vec<bf16> = combined_heads.iter().map(|&x| bf16::from_f32(x)).collect();
        let mut attn_out = vec![0.0f32; self.hidden_size];
        matvec_bf16_to_f32(o_proj_w, &combined_bf16, &mut attn_out, self.hidden_size, q_out_dim);

        attn_out
    }
}
