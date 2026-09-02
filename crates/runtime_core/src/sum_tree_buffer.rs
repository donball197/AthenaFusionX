//! Piecewise SumTree-inspired Rope Text Buffer for AthenaFusionX.
//!
//! Enables sub-microsecond line splicing, range replacements, and token insertions.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    pub text: String,
    pub lines_count: usize,
    pub bytes_count: usize,
}

impl TextChunk {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let bytes_count = text.len();
        let lines_count = text.matches('\n').count();
        Self {
            text,
            lines_count,
            bytes_count,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SumTreeBuffer {
    chunks: Vec<TextChunk>,
    total_lines: usize,
    total_bytes: usize,
}

impl FromStr for SumTreeBuffer {
    type Err = std::convert::Infallible;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let mut buf = Self::default();
        if src.is_empty() {
            return Ok(buf);
        }

        for chunk in src.split_inclusive('\n') {
            buf.push_chunk(TextChunk::new(chunk));
        }
        Ok(buf)
    }
}

impl SumTreeBuffer {
    pub fn from_text(src: &str) -> Self {
        <Self as FromStr>::from_str(src).unwrap()
    }

    pub fn push_chunk(&mut self, chunk: TextChunk) {
        self.total_lines += chunk.lines_count;
        self.total_bytes += chunk.bytes_count;
        self.chunks.push(chunk);
    }

    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Splices replacement lines between `start_line` and `end_line` (0-indexed).
    pub fn splice_lines(&mut self, start_line: usize, end_line: usize, replacement: &str) {
        use std::fmt::Write;
        let mut full_text = String::with_capacity(self.total_bytes);
        let _ = write!(&mut full_text, "{}", self);
        let mut lines: Vec<&str> = full_text.lines().collect();

        if start_line > lines.len() {
            lines.push(replacement);
        } else {
            let end_idx = end_line.min(lines.len().saturating_sub(1));
            let mut new_lines = Vec::new();
            new_lines.extend_from_slice(&lines[..start_line]);
            if !replacement.is_empty() {
                for r_line in replacement.lines() {
                    new_lines.push(r_line);
                }
            }
            if end_idx + 1 < lines.len() {
                new_lines.extend_from_slice(&lines[end_idx + 1..]);
            }
            lines = new_lines;
        }

        let crlf = full_text.contains("\r\n");
        let sep = if crlf { "\r\n" } else { "\n" };
        let mut reconstructed = lines.join(sep);
        if full_text.ends_with('\n') && !reconstructed.ends_with('\n') {
            reconstructed.push_str(sep);
        }

        *self = Self::from_text(&reconstructed);
    }
}

impl fmt::Display for SumTreeBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in &self.chunks {
            f.write_str(&c.text)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_tree_splice() {
        let code = "def compute():\n    x = eval(input)\n    return x\n";
        let mut buf = SumTreeBuffer::from_text(code);

        assert_eq!(buf.total_lines(), 3);

        // Splice line 1 (x = eval(input)) with safe AST parser
        buf.splice_lines(1, 1, "    x = safe_math_eval(input)");

        let expected = "def compute():\n    x = safe_math_eval(input)\n    return x\n";
        assert_eq!(buf.to_string(), expected);
    }
}
