//! Thin wrapper around HuggingFace's `tokenizers` crate, loading the real
//! Gemma 4 tokenizer.json shipped with the checkpoint (SentencePiece-backed,
//! not reimplemented by hand -- avoids subtle mismatches with the real
//! vocab/merges the model was trained on).

use tokenizers::Tokenizer;

pub struct NexusTokenizer {
    inner: Tokenizer,
}

impl NexusTokenizer {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let inner = Tokenizer::from_file(path).map_err(|e| e.to_string())?;
        Ok(Self { inner })
    }

    /// Encodes text to token IDs. `add_special_tokens` should generally be
    /// true for a fresh prompt (adds BOS etc. per tokenizer_config.json).
    pub fn encode(&self, text: &str, add_special_tokens: bool) -> Result<Vec<u32>, String> {
        let enc = self.inner.encode(text, add_special_tokens).map_err(|e| e.to_string())?;
        Ok(enc.get_ids().to_vec())
    }

    /// Decodes token IDs back to text. `skip_special_tokens` should
    /// generally be true when rendering output for a person to read.
    pub fn decode(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String, String> {
        self.inner.decode(ids, skip_special_tokens).map_err(|e| e.to_string())
    }
}
