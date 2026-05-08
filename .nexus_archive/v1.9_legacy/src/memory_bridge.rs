// src/memory_bridge.rs — v1.1 [TRUNCATION_COMPRESSION]
//
// FIX: summarize_memory_strand now truncates strands exceeding
//      MAX_STRAND_CHARS to prevent unbounded memory DB growth.
//      A truncation marker is appended so the LLM knows the
//      strand was compressed.
// NOTE: Full Zig pruner integration remains a future upgrade.

const MAX_STRAND_CHARS: usize = 2_000;

/// summarize_memory_strand: Compress a raw memory strand for storage.
/// Currently applies a hard character cap with a truncation marker.
/// Future: delegate to the Zig pruner substrate for semantic compression.
pub fn summarize_memory_strand(raw_text: &str) -> String {
    if raw_text.len() <= MAX_STRAND_CHARS {
        return raw_text.to_string();
    }

    // Find a clean word boundary near the limit
    let boundary = raw_text[..MAX_STRAND_CHARS]
        .rfind(|c: char| c.is_whitespace())
        .unwrap_or(MAX_STRAND_CHARS);

    format!("{}… [TRUNCATED: {} → {} chars]", &raw_text[..boundary], raw_text.len(), boundary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_passthrough() {
        let s = "hello world";
        assert_eq!(summarize_memory_strand(s), s);
    }

    #[test]
    fn test_truncation_marker() {
        let long = "x ".repeat(2000);
        let result = summarize_memory_strand(&long);
        assert!(result.contains("[TRUNCATED:"));
        assert!(result.len() <= MAX_STRAND_CHARS + 60); // marker overhead
    }
}
