//! Stage 1 Per-Layer Embeddings (PLE) for Gemma 4 E2B
//! Master table: embed_tokens_per_layer [262144, 8960] (35 layers * 256 dims).
//! Produces the base per-layer input tensor (seq, 35, 256) blended with model projections.

pub struct Stage1PLE {
    num_layers: usize,
    layer_dim: usize,
}

impl Stage1PLE {
    pub fn new(num_layers: usize, layer_dim: usize) -> Self {
        Self { num_layers, layer_dim }
    }

    /// Computes Stage 1 PLE vector for a single token ID.
    /// Extracts the 8960-dim slice from embed_tokens_per_layer and splits into 35 chunks of 256-dim.
    pub fn compute_raw_ple(
        &self,
        token_id: u32,
        embed_tokens_per_layer_weights: &[f32], // Flattened [262144 * 8960] or row view
        out_ple: &mut [f32],                    // Output buffer [35 * 256]
    ) {
        let total_dim = self.num_layers * self.layer_dim;
        assert_eq!(total_dim, 8960);
        assert_eq!(out_ple.len(), total_dim);

        let row_start = (token_id as usize) * total_dim;
        let row_end = row_start + total_dim;

        // Ensure bounds on weight slice if checking flat buffer
        if embed_tokens_per_layer_weights.len() >= row_end {
            out_ple.copy_from_slice(&embed_tokens_per_layer_weights[row_start..row_end]);
        }
    }

    /// Blends raw PLE with the projected model representation scaled by 1/sqrt(2).
    pub fn blend_stage1(&self, raw_ple: &[f32], proj: &[f32], out_blended: &mut [f32]) {
        let len = raw_ple.len();
        assert_eq!(proj.len(), len);
        assert_eq!(out_blended.len(), len);

        let scale = 1.0f32 / (2.0f32).sqrt();
        for i in 0..len {
            out_blended[i] = (proj[i] + raw_ple[i]) * scale;
        }
    }
}

/// Stage 2 Per-Layer Conditioning and Residual Integration
pub struct Stage2PLE;

impl Stage2PLE {
    /// Applies GELU activation, conditioning scale, and residual addition.
    pub fn forward_layer(hidden_states: &mut [f32], residual: &[f32], conditioning_scale: f32) {
        assert_eq!(hidden_states.len(), residual.len());
        for i in 0..hidden_states.len() {
            let x = hidden_states[i];
            // Approximate GELU activation: 0.5 * x * (1.0 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
            let inner = 0.797_884_6_f32 * (x + 0.044715_f32 * x.powi(3));
            let gelu = 0.5_f32 * x * (1.0_f32 + inner.tanh());

            // Apply conditioning scale and add residual
            hidden_states[i] = (gelu * conditioning_scale) + residual[i];
        }
    }
}
