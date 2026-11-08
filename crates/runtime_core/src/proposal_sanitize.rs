//! Strategist proposal sanitizer: strips model control-token leaks
//! (e.g. `<|end|><|start|>assistant to=read_file_chunk<|channel|>...`) from
//! string fields before the proposal reaches the critic or halt gates.

use serde_json::Value;

const LEAK_MARKERS: [&str; 2] = ["<|", "<rfl:"];

#[derive(Debug)]
pub struct Sanitized {
    pub value: Value,
    /// True if any field was cut. Callers should treat a stripped `halt` as a
    /// parse fault (re-ask, no belief penalty), not as an accepted halt.
    pub stripped: bool,
}

fn cut_at_leak(s: &str) -> Option<&str> {
    LEAK_MARKERS.iter().filter_map(|m| s.find(m)).min().map(|i| s[..i].trim_end())
}

fn walk(v: &mut Value, stripped: &mut bool) {
    match v {
        Value::String(s) => {
            if let Some(clean) = cut_at_leak(s) {
                *s = clean.to_string();
                *stripped = true;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| walk(x, stripped)),
        Value::Object(o) => o.values_mut().for_each(|x| walk(x, stripped)),
        _ => {}
    }
}

/// Strip chat-template control tokens (`<|end|>`, `<|start|>`,
/// `<|channel|>`, `<|constrain|>`, `<|message|>`, etc.) from a raw
/// string. Used by the strategist before JSON parsing so that a
/// proposal containing an accidental token leak still yields a valid
/// JSON document.
pub fn strip_control_tokens(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for `<|` ... `|>` and skip the whole span.
        if i + 1 < bytes.len() && bytes[i] == b'<' && bytes[i + 1] == b'|' {
            // Find closing `|>`.
            if let Some(end) = raw[i + 2..].find("|>") {
                i += 2 + end + 2;
                continue;
            }
        }
        out.push(raw.as_bytes()[i] as char);
        i += 1;
    }
    out
}

pub fn sanitize(mut value: Value) -> Sanitized {
    let mut stripped = false;
    walk(&mut value, &mut stripped);
    Sanitized { value, stripped }
}

/// True when every leak sits in a non-halt action's `reasoning`-style field, so
/// the executable fields (path, command, content) are intact and the sanitized
/// proposal is safe to act on.
pub fn leak_confined_to_reasoning(original: &Value) -> bool {
    let objs: Vec<&Value> = match original {
        Value::Array(a) => a.iter().collect(),
        other => vec![other],
    };
    objs.iter().all(|o| {
        let Some(m) = o.as_object() else { return false };
        if m.get("type").and_then(Value::as_str) == Some("halt") {
            return false;
        }
        m.iter()
            .filter(|(k, _)| !matches!(k.as_str(), "reasoning" | "_thought_process"))
            .all(|(_, v)| !sanitize(v.clone()).stripped)
    })
}

/// True if the proposal contains a `halt` whose text was cut by sanitizing.
pub fn stripped_halt(s: &Sanitized, original: &Value) -> bool {
    s.stripped
        && original
            .as_array()
            .is_some_and(|a| a.iter().any(|o| o.get("type").and_then(Value::as_str) == Some("halt")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_harmony_leak() {
        let v = json!([{"type":"halt","reasoning":"Need lines 1-71.<|end|><|start|>assistant to=read_file_chunk<|channel|>x"}]);
        let s = sanitize(v.clone());
        assert!(s.stripped);
        assert_eq!(s.value[0]["reasoning"], "Need lines 1-71.");
        assert!(stripped_halt(&s, &v));
    }

    #[test]
    fn clean_input_untouched() {
        let v = json!([{"type":"read_file","path":"a.rs","reasoning":"ok"}]);
        let s = sanitize(v.clone());
        assert!(!s.stripped);
        assert_eq!(s.value, v);
    }

    #[test]
    fn nested_strings_cleaned() {
        let s = sanitize(json!([{"type":"write_file","content":"fn x(){}<|channel|>junk"}]));
        assert_eq!(s.value[0]["content"], "fn x(){}");
    }
}

#[cfg(test)]
mod confined_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reasoning_only_leak_is_confined() {
        let v = json!([{"type":"read_file","path":"a.rs","reasoning":"x<|end|>junk"}]);
        assert!(leak_confined_to_reasoning(&v));
    }
    #[test]
    fn content_or_halt_leak_is_not_confined() {
        let w = json!([{"type":"write_file","path":"a.rs","content":"fn x(){}<|end|>","reasoning":"r"}]);
        assert!(!leak_confined_to_reasoning(&w));
        let h = json!([{"type":"halt","reasoning":"r<|end|>"}]);
        assert!(!leak_confined_to_reasoning(&h));
    }
}
