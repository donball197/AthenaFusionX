//! High-Level Inference Engine for Gemma 4 E2B via nexus_edge_minion
//! Coordinates safetensors model weights loading, orchestrator setup, and autoregressive generation loops.

use crate::forward_pass::ForwardOrchestrator;
use crate::decoder_layer::DecoderLayerWeights;
use crate::safetensors::EdgeLoader;
use std::path::Path;
use half::bf16;

struct CachedNormWeights {
    input_layernorm: Vec<f32>,
    post_attention_layernorm: Vec<f32>,
    pre_feedforward_layernorm: Vec<f32>,
    post_feedforward_layernorm: Vec<f32>,
    q_norm: Vec<f32>,
    k_norm: Vec<f32>,
    post_per_layer_input_norm: Vec<f32>,
}

pub struct InferenceEngine {
    orchestrator: ForwardOrchestrator,
    loader: EdgeLoader,
    cached_norms: Vec<CachedNormWeights>,
}

impl InferenceEngine {
    pub fn new<P: AsRef<Path>>(weights_path: P) -> Result<Self, &'static str> {
        let path_str = weights_path.as_ref().to_str().ok_or("invalid weights path string")?;
        let loader = EdgeLoader::new(path_str)?;
        let orchestrator = ForwardOrchestrator::new();

        // Pre-cache small normalization weights as f32 vectors once at startup
        let mut cached_norms = Vec::with_capacity(35);
        for layer_idx in 0..35 {
            let prefix = format!("model.language_model.layers.{}", layer_idx);
            
            let load_f32 = |name: &str| -> Vec<f32> {
                if let Ok(ts) = loader.get_tensor(name) {
                    if let Ok(tt) = ts.as_bf16() {
                        return tt.data.iter().map(|v| v.to_f32()).collect();
                    }
                }
                Vec::new()
            };

            cached_norms.push(CachedNormWeights {
                input_layernorm: load_f32(&format!("{}.input_layernorm.weight", prefix)),
                post_attention_layernorm: load_f32(&format!("{}.post_attention_layernorm.weight", prefix)),
                pre_feedforward_layernorm: load_f32(&format!("{}.pre_feedforward_layernorm.weight", prefix)),
                post_feedforward_layernorm: load_f32(&format!("{}.post_feedforward_layernorm.weight", prefix)),
                q_norm: load_f32(&format!("{}.self_attn.q_norm.weight", prefix)),
                k_norm: load_f32(&format!("{}.self_attn.k_norm.weight", prefix)),
                post_per_layer_input_norm: load_f32(&format!("{}.post_per_layer_input_norm.weight", prefix)),
            });
        }

        Ok(Self {
            orchestrator,
            loader,
            cached_norms,
        })
    }

    /// Runs a single autoregressive token generation step through the 35-layer orchestrator.
    pub fn step(
        &mut self,
        token_id: u32,
        position: usize,
        hidden_state: &mut [f32],
    ) -> Result<(), &'static str> {
        // Retrieve embed_tokens tensor slice matching checkpoint schema
        let tensor_slice = self.loader.get_tensor("model.language_model.embed_tokens.weight")?;
        let typed_tensor = tensor_slice.as_bf16()?;
        
        if typed_tensor.data.is_empty() {
            return Err("FATAL: Invalid embed_tokens tensor data.");
        }

        let total_row_elements = 35 * 256;
        let vocab_size = typed_tensor.shape[0] as usize;
        let hidden_dim = typed_tensor.shape[1] as usize;

        let token_idx = (token_id as usize) % vocab_size;
        let token_embedding = &typed_tensor.data[token_idx * hidden_dim .. (token_idx + 1) * hidden_dim];

        let mut f32_row = vec![0.0f32; total_row_elements];
        for (i, val) in token_embedding.iter().enumerate() {
            let target_idx = i % total_row_elements;
            f32_row[target_idx] += val.to_f32();
        }

        let get_mmap_bf16 = |name: &str| -> &[bf16] {
            if let Ok(ts) = self.loader.get_tensor(name) {
                if let Ok(tt) = ts.as_bf16() {
                    let ptr = tt.data.as_ptr();
                    let len = tt.data.len();
                    return unsafe { std::slice::from_raw_parts(ptr, len) };
                }
            }
            &[]
        };

        self.orchestrator.execute_token_step(
            hidden_state,
            &f32_row,
            position,
            |layer_idx| {
                let prefix = format!("model.language_model.layers.{}", layer_idx);
                let norms = &self.cached_norms[layer_idx];

                let gate_proj = get_mmap_bf16(&format!("{}.mlp.gate_proj.weight", prefix));
                let mlp_intermediate_size = if !gate_proj.is_empty() { gate_proj.len() / 1536 } else { 12288 };

                DecoderLayerWeights {
                    input_layernorm: &norms.input_layernorm,
                    post_attention_layernorm: &norms.post_attention_layernorm,
                    pre_feedforward_layernorm: &norms.pre_feedforward_layernorm,
                    post_feedforward_layernorm: &norms.post_feedforward_layernorm,

                    q_proj: get_mmap_bf16(&format!("{}.self_attn.q_proj.weight", prefix)),
                    q_norm: &norms.q_norm,
                    k_proj: Some(get_mmap_bf16(&format!("{}.self_attn.k_proj.weight", prefix))),
                    v_proj: Some(get_mmap_bf16(&format!("{}.self_attn.v_proj.weight", prefix))),
                    k_norm: Some(&norms.k_norm),
                    o_proj: get_mmap_bf16(&format!("{}.self_attn.o_proj.weight", prefix)),

                    gate_proj,
                    up_proj: get_mmap_bf16(&format!("{}.mlp.up_proj.weight", prefix)),
                    down_proj: get_mmap_bf16(&format!("{}.mlp.down_proj.weight", prefix)),
                    mlp_intermediate_size,

                    per_layer_input_gate: get_mmap_bf16(&format!("{}.per_layer_input_gate.weight", prefix)),
                    per_layer_projection: get_mmap_bf16(&format!("{}.per_layer_projection.weight", prefix)),
                    post_per_layer_input_norm: &norms.post_per_layer_input_norm,

                    layer_scalar: 1.0,
                }
            },
        );

        Ok(())
    }
}
