use serde::de::DeserializeOwned;

pub fn safe_parse_json<T: DeserializeOwned>(raw: &str) -> Result<T, serde_json::Error> {
    // 1. Strip rogue Markdown formatting often hallucinated by models
    let mut s = raw.trim();
    if s.starts_with("```json") {
        s = s.trim_start_matches("```json").trim_end_matches("```").trim();
    } else if s.starts_with("```") {
        s = s.trim_start_matches("```").trim_end_matches("```").trim();
    }

    // 2. Scan and repair invalid escape sequences (e.g., regex inside JSON strings)
    let mut fixed = String::with_capacity(s.len() + 10);
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next_c) = chars.peek() {
                match next_c {
                    'n' | 't' | 'r' | '"' | '\\' | '/' | 'u' | 'b' | 'f' => {
                        // Valid JSON escape sequence
                        fixed.push('\\');
                    },
                    _ => {
                        // Invalid escape sequence detected. Double-escape it to protect the parser.
                        fixed.push('\\');
                        fixed.push('\\'); 
                    }
                }
            } else {
                // Trailing backslash
                fixed.push('\\');
                fixed.push('\\');
            }
        } else {
            fixed.push(c);
        }
    }
    
    // 3. Attempt to parse the repaired string, fallback to original if structural failure
    serde_json::from_str(&fixed).or_else(|_| serde_json::from_str(s))
}
