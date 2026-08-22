import sys

path = "apps/nexus_ast_tool/src/client.rs"

old = '''        let search_start = text.find("<<<< SEARCH").ok_or_else(|| anyhow::anyhow!("Missing <<<< SEARCH in payload"))?;
        let delim = text.find("====").ok_or_else(|| anyhow::anyhow!("Missing ==== in payload"))?;
        let replace_end = text.find(">>>> REPLACE").ok_or_else(|| anyhow::anyhow!("Missing >>>> REPLACE in payload"))?;
        let search_block = text[search_start + 11..delim].trim_matches('\\n').to_string();
        let replace_block = text[delim + 4..replace_end].trim_matches('\\n').to_string();
        Ok((search_block, replace_block))
    }'''

new = '''        // Models don't reliably follow the exact marker width the prompt asks
        // for -- most SEARCH/REPLACE-trained models default to the standard
        // 7-character convention (<<<<<<< SEARCH / ======= / >>>>>>> REPLACE)
        // regardless of what's requested, and a literal .find("<<<< SEARCH")
        // (4 chars) never matches inside a run of 7 (four '<' immediately
        // followed by a space never occurs within a 7-'<' run). These helpers
        // accept any marker width instead of a hardcoded offset.
        fn find_bracket_marker(text: &str, ch: char, label: &str) -> Option<(usize, usize)> {
            let label_pos = text.find(label)?;
            let bytes = text.as_bytes();
            let mut i = label_pos;
            while i > 0 && (bytes[i - 1] as char).is_whitespace() && bytes[i - 1] != b'\\n' {
                i -= 1;
            }
            let content_start = label_pos + label.len();
            let mut marker_start = i;
            while marker_start > 0 && bytes[marker_start - 1] as char == ch {
                marker_start -= 1;
            }
            if marker_start == i {
                return None;
            }
            Some((marker_start, content_start))
        }
        fn find_separator(text: &str, ch: char, from: usize) -> Option<(usize, usize)> {
            let bytes = text.as_bytes();
            let mut i = from;
            while i < bytes.len() {
                if bytes[i] as char == ch {
                    let start = i;
                    while i < bytes.len() && bytes[i] as char == ch {
                        i += 1;
                    }
                    if i - start >= 3 {
                        return Some((start, i));
                    }
                } else {
                    i += 1;
                }
            }
            None
        }

        let (_, search_content_start) = find_bracket_marker(&text, '<', "SEARCH")
            .ok_or_else(|| anyhow::anyhow!("Missing SEARCH marker in payload"))?;
        let (delim_start, delim_end) = find_separator(&text, '=', search_content_start)
            .ok_or_else(|| anyhow::anyhow!("Missing ==== separator in payload"))?;
        let (replace_marker_start, _) = find_bracket_marker(&text, '>', "REPLACE")
            .ok_or_else(|| anyhow::anyhow!("Missing REPLACE marker in payload"))?;

        let search_block = text[search_content_start..delim_start].trim_matches('\\n').to_string();
        let replace_block = text[delim_end..replace_marker_start].trim_matches('\\n').to_string();
        Ok((search_block, replace_block))
    }'''

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: SEARCH/REPLACE marker parsing now tolerant of any marker width")
