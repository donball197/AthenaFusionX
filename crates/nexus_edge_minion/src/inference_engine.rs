//! High-Level Inference Engine for Gemma 4 E2B via nexus_edge_minion
//! Coordinates safetensors model weights loading, orchestrator setup, and autoregressive generation loops.

use crate::decoder_layer::DecoderLayerWeights;
use crate::forward_pass::ForwardOrchestrator;
use crate::safetensors::EdgeLoader;
use half::bf16;
use std::path::Path;

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
                post_attention_layernorm: load_f32(&format!(
                    "{}.post_attention_layernorm.weight",
                    prefix
                )),
                pre_feedforward_layernorm: load_f32(&format!(
                    "{}.pre_feedforward_layernorm.weight",
                    prefix
                )),
                post_feedforward_layernorm: load_f32(&format!(
                    "{}.post_feedforward_layernorm.weight",
                    prefix
                )),
                q_norm: load_f32(&format!("{}.self_attn.q_norm.weight", prefix)),
                k_norm: load_f32(&format!("{}.self_attn.k_norm.weight", prefix)),
                post_per_layer_input_norm: load_f32(&format!(
                    "{}.post_per_layer_input_norm.weight",
                    prefix
                )),
            });
        }

        Ok(Self { orchestrator, loader, cached_norms })
    }

    /// Runs a single autoregressive token generation step through the 35-layer orchestrator.
    pub fn step(
        &mut self,
        token_id: u32,
        position: usize,
        hidden_state: &mut [f32],
    ) -> Result<(), &'static str> {
        const HIDDEN: usize = 1536;
        const PLE_TOTAL: usize = 35 * 256;
        const PLE_LAYER_DIM: usize = 256;

        let embed_ts = self.loader.get_tensor("model.language_model.embed_tokens.weight")?;
        let embed_tt = embed_ts.as_bf16()?;
        if embed_tt.data.is_empty() {
            return Err("FATAL: Invalid embed_tokens tensor data.");
        }
        let vocab = embed_tt.shape[0] as usize;
        let tok = (token_id as usize) % vocab;
        let scale_h = (HIDDEN as f32).sqrt();
        for (i, v) in embed_tt.data[tok * HIDDEN..(tok + 1) * HIDDEN].iter().enumerate() {
            hidden_state[i] = v.to_f32() * scale_h;
        }
        eprintln!("[dbg] after token embed: hs[0..4]={:?}", &hidden_state[0..4]);

        let ple_ts = self.loader.get_tensor("model.language_model.embed_tokens_per_layer.weight")?;
        let ple_tt = ple_ts.as_bf16()?;
        let scale_ple = (PLE_LAYER_DIM as f32).sqrt();
        let raw_ple: Vec<f32> = ple_tt.data[tok * PLE_TOTAL..(tok + 1) * PLE_TOTAL]
            .iter()
            .map(|v| v.to_f32() * scale_ple)
            .collect();
        eprintln!(
            "[dbg] raw_ple[0..4]={:?} nonzero_count={}",
            &raw_ple[0..4],
            raw_ple.iter().filter(|v| **v != 0.0).count()
        );

        let proj_ts = self.loader.get_tensor("model.language_model.per_layer_model_projection.weight")?;
        let proj_tt = proj_ts.as_bf16()?;
        let hidden_bf16: Vec<bf16> = hidden_state.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut proj = vec![0.0f32; PLE_TOTAL];
        crate::neon_math::matvec_bf16_to_f32(proj_tt.data, &hidden_bf16, &mut proj, PLE_TOTAL, HIDDEN);
        eprintln!("[dbg] proj[0..4] pre-norm={:?}", &proj[0..4]);

        let norm_ts = self.loader.get_tensor("model.language_model.per_layer_projection_norm.weight")?;
        let norm_tt = norm_ts.as_bf16()?;
        let norm_w: Vec<f32> = norm_tt.data.iter().map(|v| v.to_f32()).collect();
        for chunk in proj.chunks_mut(PLE_LAYER_DIM) {
            crate::attention::rms_norm_in_place(chunk, &norm_w);
        }
        eprintln!("[dbg] proj[0..4] post-norm={:?}", &proj[0..4]);

        let ple1 = crate::ple::Stage1PLE::new(35, PLE_LAYER_DIM);
        let mut per_layer_inputs = vec![0.0f32; PLE_TOTAL];
        ple1.blend_stage1(&raw_ple, &proj, &mut per_layer_inputs);
        eprintln!("[dbg] per_layer_inputs[0..4]={:?}", &per_layer_inputs[0..4]);
        eprintln!("[dbg] hidden_state pre-layers[0..4]={:?}", &hidden_state[0..4]);

        let total_bytes_touched = std::cell::Cell::new(0usize);
        let tensor_fetch_count = std::cell::Cell::new(0usize);

        let get_mmap_bf16 = |name: &str| -> &[bf16] {
            if let Ok(ts) = self.loader.get_tensor(name) {
                if let Ok(tt) = ts.as_bf16() {
                    let ptr = tt.data.as_ptr();
                    let len = tt.data.len();
                    total_bytes_touched.set(total_bytes_touched.get() + len * 2); // bf16 = 2 bytes
                    tensor_fetch_count.set(tensor_fetch_count.get() + 1);
                    return unsafe { std::slice::from_raw_parts(ptr, len) };
                }
            }
            eprintln!("[dbg][WARN] tensor fetch returned empty slice for: {}", name);
            &[]
        };

        self.orchestrator.execute_token_step(hidden_state, &per_layer_inputs, position, |layer_idx| {
            let prefix = format!("model.language_model.layers.{}", layer_idx);
            let norms = &self.cached_norms[layer_idx];

            let gate_proj = get_mmap_bf16(&format!("{}.mlp.gate_proj.weight", prefix));
            let mlp_intermediate_size =
                if !gate_proj.is_empty() { gate_proj.len() / 1536 } else { 12288 };

            let q_proj = get_mmap_bf16(&format!("{}.self_attn.q_proj.weight", prefix));
            if layer_idx == 0 {
                eprintln!("[dbg] layer 0 q_proj.len()={} gate_proj.len()={}", q_proj.len(), gate_proj.len());
            }

            let layer_scalar = self
                .loader
                .get_tensor(&format!("{}.layer_scalar", prefix))
                .ok()
                .and_then(|ts| ts.as_bf16().ok())
                .and_then(|tt| tt.data.first().map(|v| v.to_f32()))
                .unwrap_or(1.0);
            if layer_idx == 0 {
                eprintln!("[dbg] layer 0 layer_scalar={}", layer_scalar);
            }

            DecoderLayerWeights {
                input_layernorm: &norms.input_layernorm,
                post_attention_layernorm: &norms.post_attention_layernorm,
                pre_feedforward_layernorm: &norms.pre_feedforward_layernorm,
                post_feedforward_layernorm: &norms.post_feedforward_layernorm,

                q_proj,
                q_norm: &norms.q_norm,
                k_proj: Some(get_mmap_bf16(&format!("{}.self_attn.k_proj.weight", prefix))),
                v_proj: Some(get_mmap_bf16(&format!("{}.self_attn.v_proj.weight", prefix))),
                k_norm: Some(&norms.k_norm),
                o_proj: get_mmap_bf16(&format!("{}.self_attn.o_proj.weight", prefix)),

                gate_proj,
                up_proj: get_mmap_bf16(&format!("{}.mlp.up_proj.weight", prefix)),
                down_proj: get_mmap_bf16(&format!("{}.mlp.down_proj.weight", prefix)),
                mlp_intermediate_size,

                per_layer_input_gate: get_mmap_bf16(&format!(
                    "{}.per_layer_input_gate.weight",
                    prefix
                )),
                per_layer_projection: get_mmap_bf16(&format!(
                    "{}.per_layer_projection.weight",
                    prefix
                )),
                post_per_layer_input_norm: &norms.post_per_layer_input_norm,

                layer_scalar,
            }
        });

        eprintln!("[dbg] hidden_state post-layers[0..4]={:?}", &hidden_state[0..4]);
        eprintln!(
            "[dbg] weight bytes touched across all layers: {} bytes ({:.2} MB) across {} tensor fetches",
            total_bytes_touched.get(),
            total_bytes_touched.get() as f64 / (1024.0 * 1024.0),
            tensor_fetch_count.get()
        );

        // Final RMSNorm before any LM-head / logits step (model.language_model.norm.weight)
        if let Ok(final_norm_ts) = self.loader.get_tensor("model.language_model.norm.weight") {
            if let Ok(final_norm_tt) = final_norm_ts.as_bf16() {
                let final_norm_w: Vec<f32> = final_norm_tt.data.iter().map(|v| v.to_f32()).collect();
                if final_norm_w.len() == hidden_state.len() {
                    crate::attention::rms_norm_in_place(hidden_state, &final_norm_w);
                    eprintln!("[dbg] hidden_state post-final-norm[0..4]={:?}", &hidden_state[0..4]);
                } else {
                    eprintln!(
                        "[dbg][WARN] final norm weight len {} != hidden_state len {}, skipping",
                        final_norm_w.len(),
                        hidden_state.len()
                    );
                }
            }
        } else {
            eprintln!("[dbg][WARN] model.language_model.norm.weight not found, skipping final norm");
        }

        Ok(())
    }

    /// Computes logits from a (post-final-norm) hidden state via the tied LM
    /// head -- config has tie_word_embeddings=true, so there is no separate
    /// output projection tensor; the head reuses embed_tokens.weight
    /// [vocab_size, hidden_size] directly as [out_dim, in_dim] for the matvec.
    /// Applies final_logit_softcapping=30.0 per config.json afterward.
    pub fn compute_logits(&self, hidden_state: &[f32]) -> Result<Vec<f32>, &'static str> {
        let embed_ts = self.loader.get_tensor("model.language_model.embed_tokens.weight")?;
        let embed_tt = embed_ts.as_bf16()?;
        let vocab = embed_tt.shape[0] as usize;
        let hidden_dim = embed_tt.shape[1] as usize;

        if hidden_dim != hidden_state.len() {
            return Err("FATAL: hidden_state length mismatch with embed_tokens hidden_dim.");
        }

        let hidden_bf16: Vec<bf16> = hidden_state.iter().map(|&v| bf16::from_f32(v)).collect();
        let mut logits = vec![0.0f32; vocab];
        crate::neon_math::matvec_bf16_to_f32(embed_tt.data, &hidden_bf16, &mut logits, vocab, hidden_dim);

        const SOFTCAP: f32 = 30.0;
        for l in logits.iter_mut() {
            *l = SOFTCAP * (*l / SOFTCAP).tanh();
        }

        Ok(logits)
    }
}
