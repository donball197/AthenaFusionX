path = "crates/nexus_edge_minion/src/inference_engine.rs"
with open(path) as f:
    src = f.read()

old = '''        } else {
            eprintln!("[dbg][WARN] model.language_model.norm.weight not found, skipping final norm");
        }

        Ok(())
    }
}'''

new = '''        } else {
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
}'''

assert old in src, "Anchor not found -- aborting, no changes written"
src = src.replace(old, new, 1)
with open(path, "w") as f:
    f.write(src)
print("Applied LM head patch OK.")
