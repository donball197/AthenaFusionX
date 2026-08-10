path = "crates/nexus_edge_minion/src/inference_engine.rs"
with open(path) as f:
    src = f.read()

changes = 0

old1 = '''            let q_proj = get_mmap_bf16(&format!("{}.self_attn.q_proj.weight", prefix));
            if layer_idx == 0 {
                eprintln!("[dbg] layer 0 q_proj.len()={} gate_proj.len()={}", q_proj.len(), gate_proj.len());
            }

            DecoderLayerWeights {'''

new1 = '''            let q_proj = get_mmap_bf16(&format!("{}.self_attn.q_proj.weight", prefix));
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

            DecoderLayerWeights {'''

assert old1 in src, "Change 1 anchor not found -- aborting, no changes written"
src = src.replace(old1, new1, 1)
changes += 1

old2 = '''                post_per_layer_input_norm: &norms.post_per_layer_input_norm,

                layer_scalar: 1.0,
            }'''

new2 = '''                post_per_layer_input_norm: &norms.post_per_layer_input_norm,

                layer_scalar,
            }'''

assert old2 in src, "Change 2 anchor not found -- aborting, no changes written"
src = src.replace(old2, new2, 1)
changes += 1

old3 = '''        eprintln!("[dbg] hidden_state post-layers[0..4]={:?}", &hidden_state[0..4]);

        Ok(())
    }
}'''

new3 = '''        eprintln!("[dbg] hidden_state post-layers[0..4]={:?}", &hidden_state[0..4]);

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
}'''

assert old3 in src, "Change 3 anchor not found -- aborting, no changes written"
src = src.replace(old3, new3, 1)
changes += 1

with open(path, "w") as f:
    f.write(src)

print(f"Applied {changes} changes OK.")
