//! Full 35-Layer Forward Pass Orchestration for Gemma 4 E2B.
//! Owns the per-layer-type KV caches and the shared-KV routing table.
//!
//! num_kv_shared_layers=20: the last 20 of 35 layers reuse an earlier
//! layer's K/V (of the same attention type) instead of computing their own.
//! With layer_types alternating 4 sliding + 1 global (full_attention at
//! 0-indexed 4,9,14,19,24,29,34), the shared region is layers 15-34;
//! sliding layers in that region source from layer 13, global layers from
//! layer 14 (both derived generally below, not hardcoded).

use crate::decoder_layer::{DecoderLayer, DecoderLayerWeights};
use crate::kv_cache::StaticKVCache;
use crate::rope::AttentionMode;
use std::collections::HashMap;

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
    /// Returns the number of tokens currently held in the KV cache at the
    /// given cache index (0..kv_boundary). Diagnostic only.
    pub fn kv_cache_len(&self, cache_idx: usize) -> Option<usize> {
        self.caches.get(cache_idx).map(|c| c.len())
    }

    /// Returns the total number of distinct KV caches (== kv_boundary).
    pub fn kv_cache_count(&self) -> usize {
        self.caches.len()
    }

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

impl Default for ForwardOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}
