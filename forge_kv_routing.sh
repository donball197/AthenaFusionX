#!/bin/bash
# forge_kv_routing.sh - settles cache & routing architecture before attention
# scoring is written: per-layer-type KV cache sizing (sliding head_dim=256 vs
# global head_dim=512) and shared-KV routing (num_kv_shared_layers=20 --
# layers 15-34 reuse an earlier layer's K/V of the same attention type
# instead of computing their own).
#
# Adds/changes:
#   - attention.rs: new project_q_only() for KV-shared layers (q_proj + q_norm
#     + RoPE only -- no k_proj/v_proj/push, since this layer doesn't own K/V).
#   - decoder_layer.rs: forward() now takes is_kv_source: bool and branches
#     between project_and_cache (source layers) and project_q_only (shared
#     layers). k_proj/v_proj/k_norm are now Option<&[..]> in
#     DecoderLayerWeights since it's UNVERIFIED whether shared layers even
#     have their own k_proj/v_proj tensors in the checkpoint at all -- pass
#     None if they don't exist, Some(..) if they do and you'd rather keep
#     them loaded (they're simply unused either way).
#   - forward_pass.rs (rewritten): owns the layer_types table, the
#     compute_kv_source_layers() routing function, and allocates ONE physical
#     StaticKVCache per KV-source layer (15 total: layers 0-14), sized
#     head_dim=256/sliding_window=512 for sliding layers, head_dim=512 for
#     global layers. Shared layers (15-34) look up their source layer's
#     cache index instead of getting their own.
#   - rope.rs: AttentionMode now also derives Hash (needed for the routing
#     HashMap).
#
# STILL OPEN / flagged, not resolved by this script:
#   - GLOBAL_MAX_CONTEXT (global-layer cache capacity) is a real capacity
#     decision I made up (8192) -- global attention must see the WHOLE
#     sequence, not just sliding_window=512, or it silently loses context.
#     Set this to whatever max generation length you actually intend.
#   - Whether the checkpoint's shared layers (15-34) literally lack
#     k_proj/v_proj/k_norm tensors, or have them but they're meant to be
#     ignored -- UNVERIFIED. Worth checking via inspect_tensors on e.g. layer
#     20 before you write the weight-loading code in inference_engine.rs.
#   - The weights_for closure signature in ForwardOrchestrator::execute_token_step
#     is a design sketch, not compile-verified -- expect possible lifetime/
#     borrow-checker friction here specifically once wired to real EdgeLoader
#     borrows, more so than the more mechanical files from the last pass.
#   - Attention scoring (QK^T/softmax/*V) is still not implemented --
#     decoder_layer.rs's attention branch remains a placeholder zero vector.

set -euo pipefail

echo ">> [NEXUS] Wiring KV-cache routing architecture..."
mkdir -p crates/nexus_edge_minion/src

# ==============================================================================
# 1. rope.rs: add Hash derive to AttentionMode (needed for routing HashMap)
# ==============================================================================
echo ">> [NEXUS] Patching rope.rs (AttentionMode: add Hash derive)..."
sed -i 's/#\[derive(Debug, Clone, Copy, PartialEq, Eq)\]\npub enum AttentionMode/#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum AttentionMode/' crates/nexus_edge_minion/src/rope.rs 2>/dev/null || true
# sed with \n in pattern is unreliable across platforms; do it with perl instead for safety.
perl -0pi -e 's/#\[derive\(Debug, Clone, Copy, PartialEq, Eq\)\]\npub enum AttentionMode/#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum AttentionMode/' crates/nexus_edge_minion/src/rope.rs

# ==============================================================================
# 2. attention.rs: add project_q_only for KV-shared layers
# ==============================================================================
echo ">> [NEXUS] Appending project_q_only() to attention.rs..."
python3 - << 'PYEOF'
import re

path = "crates/nexus_edge_minion/src/attention.rs"
with open(path) as f:
    src = f.read()

addition = '''
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
}
'''

# Replace the final closing brace of `impl AttentionBlock { ... }` with the
# addition (which re-adds that closing brace at its own end).
idx = src.rfind("\n}\n")
assert idx != -1, "could not find impl block terminator in attention.rs"
src = src[:idx] + addition
with open(path, "w") as f:
    f.write(src)
print("attention.rs patched")
PYEOF

# ==============================================================================
# 3. decoder_layer.rs: is_kv_source branching, Option<> k/v weights
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/decoder_layer.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/decoder_layer.rs
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

        let _q = if is_kv_source {
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
EOF

# ==============================================================================
# 4. forward_pass.rs: layer_types table + shared-KV routing + cache allocation
# ==============================================================================
echo ">> [NEXUS] Writing crates/nexus_edge_minion/src/forward_pass.rs..."
cat << 'EOF' > crates/nexus_edge_minion/src/forward_pass.rs
//! Full 35-Layer Forward Pass Orchestration for Gemma 4 E2B.
//! Owns the per-layer-type KV caches and the shared-KV routing table.
//!
//! num_kv_shared_layers=20: the last 20 of 35 layers reuse an earlier
//! layer's K/V (of the same attention type) instead of computing their own.
//! With layer_types alternating 4 sliding + 1 global (full_attention at
//! 0-indexed 4,9,14,19,24,29,34), the shared region is layers 15-34;
//! sliding layers in that region source from layer 13, global layers from
//! layer 14 (both derived generally below, not hardcoded).

use std::collections::HashMap;
use crate::decoder_layer::{DecoderLayer, DecoderLayerWeights};
use crate::kv_cache::StaticKVCache;
use crate::rope::AttentionMode;

pub const NUM_LAYERS: usize = 35;
pub const HIDDEN_SIZE: usize = 1536;
pub const PER_LAYER_DIM: usize = 256;
pub const NUM_KV_SHARED_LAYERS: usize = 20;
pub const SLIDING_WINDOW: usize = 512;
// Global (full_attention) layers must see the WHOLE sequence, not just the
// 512-token sliding window, or global attention silently loses context past
// this length. This is a real capacity decision -- the checkpoint doesn't
// specify a hard max context here -- set it to whatever max generation
// length you actually intend to support.
pub const GLOBAL_MAX_CONTEXT: usize = 8192;

/// Confirmed from config.json: full_attention at every 5th layer (0-indexed
/// offset 4) -- 7 global / 28 sliding of 35 total.
pub fn layer_types() -> [AttentionMode; NUM_LAYERS] {
    let mut types = [AttentionMode::Sliding; NUM_LAYERS];
    let mut i = 4;
    while i < NUM_LAYERS {
        types[i] = AttentionMode::Global;
        i += 5;
    }
    types
}

/// For each layer index, which cache index it reads/writes. Layers before
/// the shared boundary (NUM_LAYERS - NUM_KV_SHARED_LAYERS) own their cache
/// (source[i] == i, and are the only layers that ever push into it). Layers
/// at or past the boundary reuse the last prior layer's cache of the same
/// attention type -- general derivation, not hardcoded indices.
pub fn compute_kv_source_layers(types: &[AttentionMode; NUM_LAYERS]) -> [usize; NUM_LAYERS] {
    let boundary = NUM_LAYERS - NUM_KV_SHARED_LAYERS;
    let mut last_seen: HashMap<AttentionMode, usize> = HashMap::new();
    let mut source = [0usize; NUM_LAYERS];
    for i in 0..NUM_LAYERS {
        if i < boundary {
            source[i] = i;
            last_seen.insert(types[i], i);
        } else {
            source[i] = *last_seen
                .get(&types[i])
                .expect("no prior layer of this attention type before the shared-KV region");
        }
    }
    source
}

pub struct ForwardOrchestrator {
    layers: Vec<DecoderLayer>,
    kv_source: [usize; NUM_LAYERS],
    kv_boundary: usize,
    caches: Vec<StaticKVCache>, // indexed 0..kv_boundary, one per KV-source layer
}

impl ForwardOrchestrator {
    pub fn new() -> Self {
        let types = layer_types();
        let kv_source = compute_kv_source_layers(&types);
        let kv_boundary = NUM_LAYERS - NUM_KV_SHARED_LAYERS;

        let layers = (0..NUM_LAYERS)
            .map(|i| DecoderLayer::new(i, types[i] == AttentionMode::Global))
            .collect();

        let caches = (0..kv_boundary)
            .map(|i| match types[i] {
                AttentionMode::Sliding => StaticKVCache::new(SLIDING_WINDOW, 256),
                AttentionMode::Global => StaticKVCache::new(GLOBAL_MAX_CONTEXT, 512),
            })
            .collect();

        Self { layers, kv_source, kv_boundary, caches }
    }

    /// Runs one token through all 35 layers. `hidden_state` is [HIDDEN_SIZE],
    /// updated in place. `per_layer_inputs` is the full (35*256) Stage-1 PLE
    /// tensor for this token. `weights_for(layer_idx)` supplies that layer's
    /// weights -- a closure so this orchestrator doesn't own weight loading
    /// (that's inference_engine's job via EdgeLoader).
    ///
    /// NOTE: the weights_for closure signature/lifetime plumbing here is a
    /// design sketch, not compile-verified against a real EdgeLoader-backed
    /// implementation -- expect possible borrow-checker friction wiring this
    /// up for real, more so than the more mechanical files from before.
    pub fn execute_token_step<'a>(
        &mut self,
        hidden_state: &mut [f32],
        per_layer_inputs: &[f32],
        position: usize,
        mut weights_for: impl FnMut(usize) -> DecoderLayerWeights<'a>,
    ) {
        assert_eq!(hidden_state.len(), HIDDEN_SIZE);
        assert_eq!(per_layer_inputs.len(), NUM_LAYERS * PER_LAYER_DIM);

        for layer_idx in 0..NUM_LAYERS {
            let weights = weights_for(layer_idx);
            let is_kv_source = layer_idx < self.kv_boundary;
            let cache_idx = self.kv_source[layer_idx];
            let layer_ple_slice =
                &per_layer_inputs[layer_idx * PER_LAYER_DIM..(layer_idx + 1) * PER_LAYER_DIM];

            self.layers[layer_idx].forward(
                hidden_state,
                layer_ple_slice,
                &weights,
                position,
                is_kv_source,
                &mut self.caches[cache_idx],
            );
        }
    }
}
EOF

echo ""
echo ">> [NEXUS] Done. Rebuild with:"
echo "     cargo build --features local_edge"
echo ""
echo ">> [NEXUS] Still open after this pass:"
echo "     - GLOBAL_MAX_CONTEXT=8192 is a made-up placeholder -- set to your real target"
echo "     - Verify via inspect_tensors whether shared layers (e.g. layer 20) actually"
echo "       carry their own k_proj/v_proj/k_norm tensors or not"
echo "     - weights_for closure lifetime plumbing not compile-verified"
echo "     - Attention scoring (QK^T/softmax/*V) still not implemented"
echo ""
