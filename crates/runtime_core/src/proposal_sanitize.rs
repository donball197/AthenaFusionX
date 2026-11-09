//! Strategist proposal sanitizer: strips model control-token leaks
//! (e.g. `<|end|><|start|>assistant to=read_file_chunk<|channel|>...`) from
//! string fields before the proposal reaches the critic or halt gates.

use serde_json::Value;

// "assistant to=" is the residue pattern Harmony chat-template leaks leave
// behind after `strip_control_tokens` removes the `<|...|>` wrappers. Without
// it, cut_at_leak finds no marker in the post-strip value and lets
// `assistant to=read_file_chunk` reach the critic and halt gate as if it
// were valid prose. This marker is post-parse only, so cutting on it is
// always JSON-safe — the string value it lands in is already isolated.
const LEAK_MARKERS: [&str; 3] = ["<|", "<rfl:", "assistant to="];

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
    let mut rest = raw;
    while !rest.is_empty() {
        // Skip a `<|...|>` span if one starts here. `<` and `|` are single-byte
        // ASCII, so this check is codepoint-boundary safe. After finding the
        // closing `|>`, `end` is a byte offset within `rest[2..]`; the two
        // bytes of `<|` are ASCII, and `|>` is also ASCII, so the resulting
        // offset lands on a char boundary.
        if rest.starts_with("<|") {
            if let Some(end) = rest[2..].find("|>") {
                rest = &rest[2 + end + 2..];
                continue;
            }
        }
        // Emit one full UTF-8 codepoint, not one byte. Previously this used
        // `raw.as_bytes()[i] as char`, which decoded each byte of a multi-byte
        // codepoint as its Latin-1 equivalent (`°` U+00B0 read as two bytes
        // became `Â` U+00C2 followed by `°`, then got re-encoded to UTF-8 —
        // the classic double-encoding corruption). Advancing by
        // `ch.len_utf8()` keeps every subsequent `rest.starts_with` check and
        // slice on a valid char boundary.
        let ch = rest.chars().next().expect("rest is non-empty");
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
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
    fn production_pipeline_cuts_harmony_residue() {
        // Production order is strip_control_tokens -> parse -> sanitize.
        // The existing `strips_harmony_leak` test calls `sanitize` directly,
        // bypassing strip and therefore missing the case where strip already
        // removed the markers cut_at_leak needed. This test exercises the
        // real pipeline and fails if the residue `assistant to=...` survives.
        // Envelope must be an array: stripped_halt() requires original.as_array(),
        // and the existing tests use the array form for the same reason.
        let raw = r#"[{"type":"halt","reasoning":"Need lines 1-71.<|end|><|start|>assistant to=read_file_chunk<|channel|>x"}]"#;
        let stripped = strip_control_tokens(raw);
        let parsed: Value =
            serde_json::from_str(&stripped).expect("stripped output must be valid JSON");
        let s = sanitize(parsed);
        let reasoning = s.value[0]["reasoning"].as_str().unwrap();
        assert!(
            !reasoning.contains("assistant to="),
            "Harmony residue leaked through strip->parse->sanitize pipeline: {:?}",
            reasoning
        );
        assert!(s.stripped, "sanitize must report a cut for the residue case");
        let original = serde_json::from_str::<Value>(raw).unwrap();
        assert!(
            stripped_halt(&s, &original),
            "stripped_halt must flag the halt for re-ask rather than accept it"
        );
    }

    #[test]
    fn preserves_multibyte_utf8() {
        // Reproduces the corrupted `°C —` observed in the mission log.
        // Before the fix, each byte of `°` and `—` was reinterpreted as a
        // Latin-1 codepoint and re-encoded, producing `Â°C â\x80\x94`.
        assert_eq!(
            strip_control_tokens("Temperature °C — throttling"),
            "Temperature °C — throttling"
        );
        assert_eq!(strip_control_tokens("héllo wörld"), "héllo wörld");
        assert_eq!(strip_control_tokens("naïve café"), "naïve café");
        assert_eq!(strip_control_tokens("日本語テスト"), "日本語テスト");
    }

    #[test]
    fn preserves_multibyte_utf8_across_stripped_tokens() {
        // The behavior asserted here is only that multi-byte codepoints
        // survive `strip_control_tokens` when a `<|...|>` span is removed
        // nearby. The post-strip residual text is NOT asserted — that
        // depends on the token grammar and is exercised separately by
        // `strips_harmony_leak` (which goes through `sanitize`/`cut_at_leak`,
        // a more aggressive truncation path than `strip_control_tokens`).
        let out = strip_control_tokens("°C —<|end|>ok");
        assert!(out.starts_with("°C —"), "UTF-8 prefix lost: {:?}", out);
        assert!(out.ends_with("ok"), "post-token suffix lost: {:?}", out);
        assert!(!out.contains("<|"), "opening token survived: {:?}", out);
        assert!(!out.contains("|>"), "closing token survived: {:?}", out);
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
