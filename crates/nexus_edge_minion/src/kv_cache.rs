use half::bf16;

/// Fixed-capacity ring buffer for K/V.
/// One allocation at construction → no further heap growth.
pub struct StaticKVCache {
    k_data: Vec<bf16>,
    v_data: Vec<bf16>,
    max_seq_len: usize,
    head_dim: usize,
    cursor: usize,
    active_tokens: usize,
}

impl StaticKVCache {
    pub fn new(max_seq_len: usize, head_dim: usize) -> Self {
        let total = max_seq_len * head_dim;
        Self {
            k_data: vec![bf16::from_f32(0.0); total],
            v_data: vec![bf16::from_f32(0.0); total],
            max_seq_len,
            head_dim,
            cursor: 0,
            active_tokens: 0,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, k: &[bf16], v: &[bf16]) {
        debug_assert_eq!(k.len(), self.head_dim);
        debug_assert_eq!(v.len(), self.head_dim);

        let off = self.cursor * self.head_dim;
        self.k_data[off..off + self.head_dim].copy_from_slice(k);
        self.v_data[off..off + self.head_dim].copy_from_slice(v);

        self.cursor = (self.cursor + 1) % self.max_seq_len;
        if self.active_tokens < self.max_seq_len {
            self.active_tokens += 1;
        }
    }

    #[inline(always)]
    pub fn k_at(&self, pos: usize) -> &[bf16] {
        let off = pos * self.head_dim;
        &self.k_data[off..off + self.head_dim]
    }

    #[inline(always)]
    pub fn v_at(&self, pos: usize) -> &[bf16] {
        let off = pos * self.head_dim;
        &self.v_data[off..off + self.head_dim]
    }

    pub fn len(&self) -> usize {
        self.active_tokens
    }

    pub fn capacity(&self) -> usize {
        self.max_seq_len
    }
}
