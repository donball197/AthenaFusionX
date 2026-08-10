import re

path = "crates/nexus_edge_minion/src/inference_engine.rs"
with open(path) as f:
    src = f.read()

old_start = src.index("    pub fn step(")
old_end_marker = "        Ok(())\n    }\n}"
old_end = src.index(old_end_marker, old_start) + len(old_end_marker)
old_block = src[old_start:old_end]

new_block = '''    pub fn step(
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

                layer_scalar: 1.0,
            }
        });

        eprintln!("[dbg] hidden_state post-layers[0..4]={:?}", &hidden_state[0..4]);

        Ok(())
    }
}'''

assert old_block in src, "old_block not found verbatim -- aborting, no changes written"
src2 = src.replace(old_block, new_block)
with open(path, "w") as f:
    f.write(src2)
print("Patched OK. Bytes before:", len(old_block), "after:", len(new_block))
