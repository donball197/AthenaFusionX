#!/bin/bash
# forge_attention_scoring.sh - Implements real scaled dot-product attention
# and output projection (o_proj) for Gemma 4 E2B MQA in nexus_edge_minion.
#
# Adds:
#   - compute_attention_output() in attention.rs:
#       1. Iterates through all Q heads (8 heads * head_dim).
#       2. Computes dot products Q_h . K_pos / sqrt(head_dim) against all cached positions (MQA single KV head).
#       3. Applies numerically stable softmax over historical sequence positions.
#       4. Aggregates weighted values V_pos into head output buffers.
#       5. Runs combined head outputs through o_proj using neon_math::matvec_bf16_to_f32.
#   - Updates decoder_layer.rs to replace the placeholder zero vector with real attention output.

set -euo pipefail

echo ">> [NEXUS] Wiring Attention Scoring & Output Projection (o_proj)..."

# ==============================================================================
# 1. attention.rs: Add compute_attention_output()
# ==============================================================================
echo ">> [NEXUS] Patching attention.rs with compute_attention_output()..."
python3 - << 'PYEOF'
import sys

path = "crates/nexus_edge_minion/src/attention.rs"
with open(path, "r") as f:
    src = f.read()

addition = '''
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
'''

idx = src.rfind("\n}\n")
assert idx != -1, "Could not find impl AttentionBlock terminator in attention.rs"
src = src[:idx] + addition
with open(path, "w") as f:
    f.write(src)
print(">> [NEXUS] attention.rs successfully patched with compute_attention_output().")
PYEOF

# ==============================================================================
# 2. decoder_layer.rs: Connect attention scoring in forward pass
# ==============================================================================
echo ">> [NEXUS] Patching decoder_layer.rs to execute real attention scoring..."
python3 - << 'PYEOF'
path = "crates/nexus_edge_minion/src/decoder_layer.rs"
with open(path, "r") as f:
    src = f.read()

# Replace placeholder vector with real attention call
old_code = "let _q = if is_kv_source {"
new_code = "let q = if is_kv_source {"
src = src.replace(old_code, new_code)

old_placeholder = "let mut attn_out = vec![0.0f32; HIDDEN_SIZE];"
new_attn_call = "let mut attn_out = self.attn.compute_attention_output(&q, weights.o_proj, kv_cache);"
src = src.replace(old_placeholder, new_attn_call)

with open(path, "w") as f:
    f.write(src)
print(">> [NEXUS] decoder_layer.rs successfully patched.")
PYEOF

echo ""
echo ">> [NEXUS] Done! Build and test with:"
echo "     cargo build --features local_edge"
echo ""
