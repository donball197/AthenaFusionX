//! Deterministic check of a HALT report's backticked symbol claims against
//! the evidence the session actually read. Pure: no logging, no I/O.

const SKIP: &[&str] = &[
    "u8", "u16", "u32", "u64", "usize", "i32", "i64", "f32", "f64", "bool", "str",
    "self", "Self", "mut", "true", "false", "async", "await", "fn", "pub", "let",
    "None", "Some", "Ok", "Err",
];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_ident(s: &str) -> bool {
    let mut it = s.chars();
    matches!(it.next(), Some(c) if c.is_alphabetic() || c == '_') && it.all(is_word_char)
}

fn word_present(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(i, _)| {
        let before = hay[..i].chars().next_back();
        let after = hay[i + needle.len()..].chars().next();
        !before.map_or(false, is_word_char) && !after.map_or(false, is_word_char)
    })
}

/// Evidence minus self-referential halt lines, so a report (or a prior
/// rejection message) can never verify its own claims.
pub fn verification_corpus(local_evidence: &str, current_turn_evidence: &str) -> String {
    local_evidence
        .replace(current_turn_evidence, "")
        .lines()
        .filter(|l| !l.starts_with("[HALT"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Backticked identifiers in `report` that do not appear in `corpus`.
/// CamelCase / CONSTANT -> must appear as a whole word.
/// snake_case           -> must appear as `fn name` or `name(`.
/// Spans with spaces, paths, attributes, or punctuation are ignored.
pub fn unverified_symbols(report: &str, corpus: &str) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    for span in report.split('`').skip(1).step_by(2) {
        let span = span.trim().trim_end_matches("()");
        let name = span.rsplit("::").next().unwrap_or("");
        if !is_ident(name) || SKIP.contains(&name) {
            continue;
        }
        let found = if name.chars().next().map_or(false, |c| c.is_uppercase()) {
            word_present(corpus, name)
        } else {
            word_present(corpus, &format!("fn {}", name))
                || corpus.contains(&format!("{}(", name))
                // Variables, fields and string literals (e.g. `mutation_paths`,
                // `python3`) are legitimate backticked claims; they only need to
                // appear in the evidence. Invented names still fail.
                || word_present(corpus, name)
        };
        if !found && !bad.iter().any(|b| b == name) {
            bad.push(name.to_string());
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "pub async fn ignite(&mut self) {}\nasync fn handle_execute() {}\nenum PatchClass { Mutation }\n";

    #[test]
    fn flags_invented_symbols() {
        let r = "`CorticalLoop::run` / `step` and `PatchClass`";
        assert_eq!(unverified_symbols(r, SRC), vec!["run", "step"]);
    }

    #[test]
    fn accepts_real_symbols() {
        let r = "`ignite`, `handle_execute()`, `PatchClass::Mutation`";
        assert!(unverified_symbols(r, SRC).is_empty());
    }

    #[test]
    fn accepts_variables_and_string_literals_present_in_source() {
        let src = format!("{}let mut mutation_paths = Vec::new();\nCommand::new(\"python3\");\n", SRC);
        assert!(unverified_symbols("`mutation_paths`, `python3`", &src).is_empty());
        assert_eq!(
            unverified_symbols("`never_defined_anywhere`", &src),
            vec!["never_defined_anywhere"]
        );
    }

    #[test]
    fn report_cannot_self_verify() {
        let halt = "[HALT] uses `step`\n";
        let ev = format!("{}{}", SRC, halt);
        let corpus = verification_corpus(&ev, halt);
        assert_eq!(unverified_symbols("`step`", &corpus), vec!["step"]);
    }
}

fn find_reasoning(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(s) = m.get("reasoning").and_then(|x| x.as_str()) {
                return Some(s.to_string());
            }
            m.values().find_map(find_reasoning)
        }
        _ => None,
    }
}

/// `reasoning` field of the serialized halt action (last line); raw text if not JSON.
pub fn report_text(executed_action: &str) -> String {
    let last = executed_action.lines().last().unwrap_or("");
    serde_json::from_str::<serde_json::Value>(last)
        .ok()
        .and_then(|v| find_reasoning(&v))
        .unwrap_or_else(|| executed_action.to_string())
}

const INTENT_PHRASES: &[&str] = &[
    "i must read", "i will now", "i will produce", "i must now",
    "has not been read", "have only read", "before halting",
];

/// Defects that make a halt text a non-report. Empty = acceptable.
pub fn report_defects(report: &str, corpus: &str) -> Vec<String> {
    let mut out = Vec::new();
    let n = report.chars().count();
    if n < 600 {
        out.push(format!("report is only {} chars; need >= 600 of findings", n));
    }
    let lower = report.to_ascii_lowercase();
    if INTENT_PHRASES.iter().any(|p| lower.contains(p)) {
        out.push("states intent/process instead of findings".to_string());
    }
    let mut verified: Vec<&str> = Vec::new();
    for span in report.split('`').skip(1).step_by(2) {
        let name = span.trim().trim_end_matches("()").rsplit("::").next().unwrap_or("");
        if is_ident(name)
            && name.chars().next().map_or(false, |c| c.is_lowercase() || c == '_')
            && word_present(corpus, &format!("fn {}", name))
            && !verified.contains(&name)
        {
            verified.push(name);
        }
    }
    if verified.len() < 3 {
        out.push(format!("only {} verified `fn` names cited; need >= 3 (enclose each function name in backticks, e.g. `fn_name`)", verified.len()));
    }
    out
}

#[cfg(test)]
mod substance_tests {
    use super::*;
    const SRC: &str = "async fn handle_strategize() {}\nasync fn handle_critic() {}\nasync fn handle_execute() {}\n";

    #[test]
    fn rejects_intent_statement() {
        let r = "The critic vetoes are correct - I must read the remaining chunk before halting. `classify_patch_action`";
        assert!(!report_defects(r, SRC).is_empty());
    }

    #[test]
    fn accepts_substantive_report() {
        let body = "`handle_strategize` selects the strategy and proposes actions. ".repeat(10)
            + "`handle_critic` filters duplicates and vetoes. `handle_execute` runs the gated motor command.";
        assert!(report_defects(&body, SRC).is_empty());
    }

    #[test]
    fn extracts_reasoning() {
        assert_eq!(report_text(r#"{"type":"halt","reasoning":"abc"}"#), "abc");
    }
}

// ---------- read-ledger line map, function outline, range verification ----------
use std::collections::BTreeMap;

fn parse_lines_header(s: &str) -> Option<(usize, usize)> {
    let i = s.find("Lines ")? + "Lines ".len();
    let rest = &s[i..];
    let a: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest2 = rest[a.len()..].strip_prefix('-')?;
    let b: String = rest2.chars().take_while(|c| c.is_ascii_digit()).collect();
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Absolute line number -> text, rebuilt from READ_OK / READ_CHUNK_OK records only.
pub fn read_lines(evidence: &str) -> BTreeMap<usize, String> {
    #[derive(Clone, Copy)]
    enum St {
        Idle,
        Expect(usize, Option<usize>),
        Body(usize, Option<usize>),
    }
    let mut map: BTreeMap<usize, String> = BTreeMap::new();
    let mut st = St::Idle;
    for line in evidence.lines() {
        if let Some(rest) = line.strip_prefix("[READ_CHUNK_OK] File:") {
            st = match parse_lines_header(rest) {
                Some((s, e)) => St::Expect(s, Some(e)),
                None => St::Idle,
            };
            continue;
        }
        if line.starts_with("[READ_OK]") {
            st = St::Expect(1, None);
            continue;
        }
        if line.starts_with("[PROPOSITION]") || line.starts_with("[CONTENT TRUNCATED") {
            st = St::Idle;
            continue;
        }
        match st {
            St::Idle => {}
            St::Expect(s, e) => {
                let t = line.trim();
                st = if t == "CONTENT:" || t == "FULL FILE CONTENT:" { St::Body(s, e) } else { St::Idle };
            }
            St::Body(n, e) => {
                if e.map_or(false, |e| n > e) {
                    st = St::Idle;
                } else {
                    map.entry(n).or_insert_with(|| line.to_string());
                    st = St::Body(n + 1, e);
                }
            }
        }
    }
    map
}

#[derive(Debug, Clone)]
pub struct FnSpan {
    pub name: String,
    pub start: usize,
    pub end: Option<usize>,
}

fn fn_name_on(text: &str) -> Option<String> {
    let mut t = text.trim_start();
    loop {
        let mut changed = false;
        for p in ["pub(crate) ", "pub(super) ", "pub ", "async ", "unsafe ", "const "].iter() {
            if let Some(r) = t.strip_prefix(*p) {
                t = r;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let rest = t.strip_prefix("fn ")?;
    let name: String = rest.chars().take_while(|c| is_word_char(*c)).collect();
    if name.is_empty() { None } else { Some(name) }
}

fn indent_of(s: &str) -> usize {
    s.len() - s.trim_start().len()
}

pub fn function_spans(lines: &BTreeMap<usize, String>) -> Vec<FnSpan> {
    let mut out = Vec::new();
    for (&n, text) in lines.iter() {
        let name = match fn_name_on(text) {
            Some(x) => x,
            None => continue,
        };
        let ind = indent_of(text);
        let tr = text.trim_end();
        if tr.ends_with('}') && tr.contains('{') {
            out.push(FnSpan { name, start: n, end: Some(n) });
            continue;
        }
        let mut end = None;
        let mut m = n + 1;
        while let Some(l) = lines.get(&m) {
            if indent_of(l) == ind && l.trim_start().starts_with('}') {
                end = Some(m);
                break;
            }
            m += 1;
        }
        out.push(FnSpan { name, start: n, end });
    }
    out
}

pub fn function_outline(evidence: &str, min_lines: usize) -> String {
    let lines = read_lines(evidence);
    let mut spans: Vec<FnSpan> = function_spans(&lines)
        .into_iter()
        .filter(|s| s.end.map_or(false, |e| e + 1 - s.start >= min_lines))
        .collect();
    if spans.is_empty() {
        return String::new();
    }
    spans.sort_by_key(|s| s.start);
    let mut out = String::from(
        "[AUTHORITATIVE FUNCTION OUTLINE - computed by the Cortex from the physical read ledger. Cite these exact line ranges; do not estimate.]\n",
    );
    for s in spans.iter().take(60) {
        let e = s.end.unwrap_or(s.start);
        out.push_str(&format!("- `{}`: lines {}-{} ({} lines)\n", s.name, s.start, e, e + 1 - s.start));
    }
    out
}

fn words_at(hay: &str, needle: &str) -> Vec<usize> {
    hay.match_indices(needle)
        .filter(|&(i, _)| {
            let before = hay[..i].chars().next_back();
            let after = hay[i + needle.len()..].chars().next();
            !before.map_or(false, is_word_char) && !after.map_or(false, is_word_char)
        })
        .map(|(i, _)| i)
        .collect()
}

fn range_after(window: &str) -> Option<(usize, usize)> {
    let i = window.find("ines ")? + "ines ".len();
    let rest = window[i..].trim_start_matches(|c: char| !c.is_ascii_digit());
    let a: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if a.is_empty() {
        return None;
    }
    let mut r = &rest[a.len()..];
    for sep in ["-", "\u{2013}", "\u{2014}", " to ", " - "].iter() {
        if let Some(x) = r.strip_prefix(*sep) {
            r = x;
            break;
        }
    }
    let b: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
    if b.is_empty() {
        return None;
    }
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Cited `name` + "Lines A-B" on the same report line must match the ledger.
pub fn range_defects(report: &str, spans: &[FnSpan], min_len: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for sp in spans {
        let end = match sp.end {
            Some(e) => e,
            None => continue,
        };
        if end + 1 - sp.start < min_len || seen.contains(&sp.name.as_str()) {
            continue;
        }
        seen.push(sp.name.as_str());
        for pos in words_at(report, &sp.name) {
            let line_start = report[..pos].rfind('\n').map_or(0, |i| i + 1);
            if report[line_start..pos].matches('`').count() % 2 == 0 {
                continue;
            }
            let line: &str = report[pos..].lines().next().unwrap_or("");
            let window: String = line.chars().take(300).collect();
            if let Some((a, b)) = range_after(&window) {
                let ok = spans.iter().filter(|x| x.name == sp.name).any(|x| {
                    x.end.map_or(false, |e| a.abs_diff(x.start) <= 25 && b.abs_diff(e) <= 40)
                });
                if !ok {
                    out.push(format!(
                        "`{}` cited at lines {}-{} but the read ledger shows lines {}-{}",
                        sp.name, a, b, sp.start, end
                    ));
                }
                break;
            }
        }
    }
    out
}

/// All deterministic defects of a read-only halt report. Empty = acceptable.
pub fn halt_defects(report: &str, local_evidence: &str) -> Vec<String> {
    let lines = read_lines(local_evidence);
    let corpus = if lines.is_empty() {
        verification_corpus(local_evidence, "")
    } else {
        lines.values().cloned().collect::<Vec<_>>().join("\n")
    };
    let spans = function_spans(&lines);
    let mut out = Vec::new();
    let bad = unverified_symbols(report, &corpus);
    if !bad.is_empty() {
        out.push(format!("symbols not found in the file content read: {}", bad.join(", ")));
    }
    out.extend(report_defects(report, &corpus));
    out.extend(range_defects(report, &spans, 20));
    out
}

#[cfg(test)]
mod outline_tests {
    use super::*;
    const EV: &str = "[READ_CHUNK_OK] File: x.rs | Lines 10-19 of 40\nCONTENT:\nfn alpha() {\n    1;\n}\n\nasync fn beta(\n    a: u8,\n) -> u8 {\n    a\n}\n\n[PROPOSITION] File: -> x\n[READ_CHUNK_OK] File: x.rs | Lines 10-19 of 40\n[PROPOSITION] File: -> x\n";

    #[test]
    fn numbers_lines_from_chunk_headers() {
        let m = read_lines(EV);
        assert_eq!(m.get(&10).map(String::as_str), Some("fn alpha() {"));
        assert_eq!(m.get(&18).map(String::as_str), Some("}"));
    }

    #[test]
    fn computes_function_spans() {
        let sp = function_spans(&read_lines(EV));
        let a = sp.iter().find(|s| s.name == "alpha").unwrap();
        let b = sp.iter().find(|s| s.name == "beta").unwrap();
        assert_eq!((a.start, a.end), (10, Some(12)));
        assert_eq!((b.start, b.end), (14, Some(18)));
    }

    #[test]
    fn flags_wrong_ranges_and_accepts_right_ones() {
        let sp = function_spans(&read_lines(EV));
        assert!(range_defects("- `beta`: Lines 14-18", &sp, 1).is_empty());
        assert!(range_defects("- `beta` (Lines 12-20)", &sp, 1).is_empty());
        assert_eq!(range_defects("- `beta`: Lines 100-180", &sp, 1).len(), 1);
    }

    #[test]
    fn outline_lists_functions() {
        let o = function_outline(EV, 3);
        assert!(o.contains("`beta`: lines 14-18"));
        assert!(o.contains("`alpha`: lines 10-12"));
    }
}
