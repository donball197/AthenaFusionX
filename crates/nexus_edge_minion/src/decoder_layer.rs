//! Full Gemma4DecoderLayer sandwich: 4 RMSNorms, self-attention setup, MLP,
//! PLE stage 2 injection, layer_scalar.
//!
//! NOTE: the actual QK^T / softmax / *V attention score computation is not
//! yet implemented -- that's the next roadmap milestone. Until it exists,
//! the attention branch's contribution is a placeholder zero vector.

use half::bf16;
use crate::attention::{AttentionBlock, rms_norm_in_place};
use crate::kv_cache::StaticKVCache;
use crate::neon_math::matvec_bf16_to_f32;

const HIDDEN_SIZE: usize = 1536;
const PER_LAYER_DIM: usize = 256;

/// Borrowed per-layer weights for one forward call. k_proj/v_proj/k_norm are
/// Option<> because it's UNVERIFIED whether KV-shared layers (the last
/// num_kv_shared_layers) even carry their own copies of these tensors in the
/// checkpoint -- pass None for shared layers regardless of which is true,
/// since they go unused either way.
pub struct DecoderLayerWeights<'a> {
    pub input_layernorm: &'a [f32],
    pub post_attention_layernorm: &'a [f32],
    pub pre_feedforward_layernorm: &'a [f32],
    pub post_feedforward_layernorm: &'a [f32],

    pub q_proj: &'a [bf16],
    pub q_norm: &'a [f32],
    pub k_proj: Option<&'a [bf16]>,
    pub v_proj: Option<&'a [bf16]>,
    pub k_norm: Option<&'a [f32]>,
    pub o_proj: &'a [bf16],

    pub gate_proj: &'a [bf16],
    pub up_proj: &'a [bf16],
    pub down_proj: &'a [bf16],
    pub mlp_intermediate_size: usize, // 6144

    pub per_layer_input_gate: &'a [bf16],
    pub per_layer_projection: &'a [bf16],
    pub post_per_layer_input_norm: &'a [f32],

    // Confirmed real (HF transformers issue #45482) but exact application
    // point unverified against primary source.
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
    /// 256-dim slice of the Stage-1 per_layer_inputs tensor.
    ///
    /// `is_kv_source`: true if this layer computes and pushes its own K/V
    /// into `kv_cache`; false if this layer only projects Q and relies on
    /// `kv_cache` already holding this position's K/V (pushed earlier this
    /// same token-step by the actual source layer -- caller must pass the
    /// SAME StaticKVCache instance for both, per the routing table).
    pub fn forward(
        &self,
        hidden_state: &mut [f32],
        layer_ple_slice: &[f32],
        weights: &DecoderLayerWeights,
        position: usize,
        is_kv_source: bool,
        kv_cache: &mut StaticKVCache,
    ) {
        assert_eq!(hidden_state.len(), HIDDEN_SIZE);
        assert_eq!(layer_ple_slice.len(), PER_LAYER_DIM);

        // ---- Attention block ----
        let residual = hidden_state.to_vec();
        let mut x = hidden_state.to_vec();
        rms_norm_in_place(&mut x, weights.input_layernorm);

        let q = if is_kv_source {
            let k_proj = weights.k_proj.expect("is_kv_source=true requires k_proj");
            let v_proj = weights.v_proj.expect("is_kv_source=true requires v_proj");
            let k_norm = weights.k_norm.expect("is_kv_source=true requires k_norm");
            self.attn.project_and_cache(
                &x, weights.q_proj, k_proj, v_proj, weights.q_norm, k_norm, position, kv_cache,
            )
        } else {
            self.attn.project_q_only(&x, weights.q_proj, weights.q_norm, position)
        };

        // TODO(roadmap item 2): compute_attention_output(&_q, kv_cache) ->
        // [num_heads*head_dim] via softmax(QK^T/sqrt(head_dim))*V, then
        // o_proj back to HIDDEN_SIZE. Placeholder zero vector until then.
        let mut attn_out = self.attn.compute_attention_output(&q, weights.o_proj, kv_cache);

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
        let x_bf16: Vec<bf16> = hidden_state.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut ple_gate = vec![0.0f32; PER_LAYER_DIM];
        matvec_bf16_to_f32(weights.per_layer_input_gate, &x_bf16, &mut ple_gate, PER_LAYER_DIM, HIDDEN_SIZE);
        for g in ple_gate.iter_mut() {
            *g = gelu_tanh(*g);
        }

        let mut injected = vec![0.0f32; PER_LAYER_DIM];
        for i in 0..PER_LAYER_DIM {
            injected[i] = ple_gate[i] * layer_ple_slice[i];
        }

        let injected_bf16: Vec<bf16> = injected.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut ple_out = vec![0.0f32; HIDDEN_SIZE];
        matvec_bf16_to_f32(weights.per_layer_projection, &injected_bf16, &mut ple_out, HIDDEN_SIZE, PER_LAYER_DIM);
        rms_norm_in_place(&mut ple_out, weights.post_per_layer_input_norm);
        for i in 0..HIDDEN_SIZE {
            hidden_state[i] += ple_out[i];
        }

        // ---- layer_scalar ----
        for v in hidden_state.iter_mut() {
            *v *= weights.layer_scalar;
        }
    }
}

fn gelu_tanh(x: f32) -> f32 {
    let inner = 0.7978845608_f32 * (x + 0.044715_f32 * x.powi(3));
    0.5 * x * (1.0 + inner.tanh())
}
