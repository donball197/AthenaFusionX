//! Deterministic check of a HALT report's backticked symbol claims against
//! the evidence the session actually read. Pure: no logging, no I/O.
//!
//! POLICY: bare `fn_name` citations are valid when the symbol appears in
//! the READ_* verification corpus. Only explicit "lines X-Y" claims are
//! required to match the coverage ledger spans.

const SKIP: &[&str] = &[
    "u8",
    "u16",
    "u32",
    "u64",
    "usize",
    "i32",
    "i64",
    "f32",
    "f64",
    "bool",
    "str",
    "self",
    "Self",
    "mut",
    "true",
    "false",
    "async",
    "await",
    "fn",
    "pub",
    "let",
    "None",
    "Some",
    "Ok",
    "Err",
    // Source-code search markers. A halt report that documents "no TODO
    // matches found" is citing the search term, not claiming TODO exists
    // in the code. Treating these as unverified symbols rejects every
    // absence-documentation paragraph, which is the opposite of what the
    // stub-scan objective asks for.
    "TODO",
    "FIXME",
    "XXX",
    "HACK",
    "NOTE",
    "todo",
    "fixme",
    "xxx",
    "hack",
    "note",
    "todo!",
    "unimplemented!",
    "unreachable!",
    // Rust keywords: a report documenting "no `unsafe` blocks" cites the
    // keyword, it does not claim a symbol named `unsafe` exists.
    "unsafe",
    "static",
    "const",
    "struct",
    "enum",
    "impl",
    "trait",
    "mod",
    "use",
    "crate",
    "dyn",
    "ref",
    "move",
    "match",
    "loop",
    "where",
    "type",
    "extern",
    "super",
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
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
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

/// True when the backticked span sits in an absence claim ("no `x` exists",
/// "`x` does not appear"), where citing a missing symbol is the finding.
fn absence_context(report: &str, start: usize, end: usize) -> bool {
    let before_all = report.get(..start).unwrap_or("");
    let line_start = before_all.rfind('\n').map_or(0, |i| i + 1);
    let tail: Vec<char> = before_all[line_start..].chars().collect();
    let skip = tail.len().saturating_sub(60);
    let before: String = tail[skip..].iter().collect::<String>().to_lowercase();
    let before = before.trim_end().trim_end_matches('`').trim_end().to_string();
    let after: String =
        report.get(end..).unwrap_or("").chars().take(70).collect::<String>().to_lowercase();
    ["no", "not", "never", "without", "neither", "nor", "absent"]
        .iter()
        .any(|w| before.ends_with(w))
        || [
            "does not",
            "not appear",
            "not found",
            "not present",
            "never ",
            "is absent",
            "doesn't",
            "no such",
            "not exist",
        ]
        .iter()
        .any(|w| after.contains(w))
}

/// Backticked identifiers in `report` that do not appear in `corpus`.
/// CamelCase / CONSTANT -> must appear as a whole word.
/// snake_case           -> must appear as `fn name` or `name(`.
/// Spans with spaces, paths, attributes, or punctuation are ignored.
pub fn unverified_symbols(report: &str, corpus: &str) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    let mut off = 0usize;
    for (part_idx, part) in report.split('`').enumerate() {
        let span_start = off;
        off += part.len() + 1;
        if part_idx % 2 == 0 {
            continue;
        }
        let span_end = span_start + part.len();
        let span = part.trim().trim_end_matches("()");
        let name = span.rsplit("::").next().unwrap_or("");
        if !is_ident(name) || SKIP.contains(&name) || is_grounding_skip_token(name) {
            continue;
        }
        // General Markdown Formatting Fix:
        // A backticked word is considered verified if it appears anywhere in the
        // physical evidence (e.g. cargo check output, variable names, internal functions).
        // It does not need to be a formal `pub` definition.
        let found = word_present(corpus, name);
        if !found
            && !absence_context(report, span_start, span_end)
            && !bad.iter().any(|b| b == name)
        {
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
        let src =
            format!("{}let mut mutation_paths = Vec::new();\nCommand::new(\"python3\");\n", SRC);
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
    "i must read",
    "i will now",
    "i will produce",
    "i must now",
    "has not been read",
    "have only read",
    "before halting",
];

const META_STUB_PHRASES: &[&str] = &[
    "in the halt reasoning",
    "halt reasoning field",
    "report has been synthesized",
    "has been synthesized inline",
    "mandatory node success criteria have been",
    "evidence is sufficient to answer",
    "halting with the inline report",
];

/// Defects that make a halt text a non-report. Empty = acceptable.
pub fn report_defects(report: &str, corpus: &str) -> Vec<String> {
    let mut out = Vec::new();
    let n = report.chars().count();
    if n < 50 {
        out.push(format!("report is only {} chars; need >= 50 of findings", n));
    }
    let lower = report.to_ascii_lowercase();
    if INTENT_PHRASES.iter().any(|p| lower.contains(p)) {
        out.push("states intent/process instead of findings".to_string());
    }
    let mut verified_any: Vec<&str> = Vec::new();
    for span in report.split('`').skip(1).step_by(2) {
        let name = span.trim().trim_end_matches("()").rsplit("::").next().unwrap_or("");
        if is_ident(name)
            && !SKIP.contains(&name)
            && !is_grounding_skip_token(name)
            && word_present(corpus, name)
            && !verified_any.contains(&name)
        {
            verified_any.push(name);
        }
    }
    if META_STUB_PHRASES.iter().any(|p| lower.contains(p))
        && (verified_any.is_empty()
            || (verified_any.len() <= 1 && lower.contains("no further actions are needed")))
    {
        out.push(
            "meta-summary claims a report exists without delivering inline findings"
                .to_string(),
        );
    }
    if verified_any.is_empty() && corpus.len() > 1000 && corpus.contains("fn ") {
        out.push(
            "no verified source identifiers cited in backticks; deliver concrete findings citing symbols from the read evidence (e.g. `fn_name` or `StructName`)"
                .to_string(),
        );
    }
    // Large evidence corpus: a one-symbol, two-paragraph stub is not an audit.
    if corpus.len() > 20_000 {
        if verified_any.len() < 3 {
            out.push(format!(
                "only {} distinct verified symbol(s) cited; a report over this much read evidence must cite at least 3",
                verified_any.len()
            ));
        }
        if report.chars().count() < 2000 {
            out.push(
                "report is too short for the amount of evidence read; deliver the full structured findings, not a summary"
                    .to_string(),
            );
        }
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

    #[test]
    fn rejects_empty_meta_stub_on_source_audit() {
        let big_src = format!("pub struct Verifier;\npub fn verify_contract() {{}}\n{}", "x\n".repeat(600));
        let stub = "All mandatory node success criteria have been physically evidenced. The final report has been synthesized inline in the halt reasoning field. No further actions are needed.";
        assert!(!report_defects(stub, &big_src).is_empty());
        let grounded = "The `Verifier` struct checks contracts in `verify_contract` across all branches and states.";
        assert!(report_defects(grounded, &big_src).is_empty());
    }

    #[test]
    fn large_corpus_rejects_short_or_thin_reports() {
        let corpus = format!(
            "pub struct Verifier;\npub struct Gate;\npub fn verify_contract() {{}}\n{}",
            "x\n".repeat(15_000)
        );
        let stub = "The `Verifier` struct checks contracts in `verify_contract`.";
        assert!(!report_defects(stub, &corpus).is_empty());
        let full = format!(
            "`Verifier` calls `verify_contract` and `Gate` blocks. {}",
            "Detailed finding about the control flow. ".repeat(60)
        );
        assert!(report_defects(&full, &corpus).is_empty(), "{:?}", report_defects(&full, &corpus));
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
                st = if t == "CONTENT:" || t == "FULL FILE CONTENT:" {
                    St::Body(s, e)
                } else {
                    St::Idle
                };
            }
            St::Body(n, e) => {
                if e.is_some_and(|e| n > e) {
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
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
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
        .filter(|s| s.end.is_some_and(|e| e + 1 - s.start >= min_lines))
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
        out.push_str(&format!(
            "- `{}`: lines {}-{} ({} lines)\n",
            s.name,
            s.start,
            e,
            e + 1 - s.start
        ));
    }
    out
}

fn words_at(hay: &str, needle: &str) -> Vec<usize> {
    hay.match_indices(needle)
        .filter(|&(i, _)| {
            let before = hay[..i].chars().next_back();
            let after = hay[i + needle.len()..].chars().next();
            !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
        })
        .map(|(i, _)| i)
        .collect()
}

#[allow(dead_code)]
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

fn all_ranges(s: &str) -> Vec<(usize, usize)> {
    let cs: Vec<char> = s.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0usize);
    while i < cs.len() {
        if cs[i].is_ascii_digit() {
            let st = i;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            let a: String = cs[st..i].iter().collect();
            if i + 1 < cs.len()
                && matches!(cs[i], '-' | '\u{2013}' | '\u{2014}')
                && cs[i + 1].is_ascii_digit()
            {
                let mut j = i + 1;
                while j < cs.len() && cs[j].is_ascii_digit() {
                    j += 1;
                }
                let b: String = cs[i + 1..j].iter().collect();
                if let (Ok(a), Ok(b)) = (a.parse(), b.parse()) {
                    out.push((a, b));
                }
                i = j;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// All ranges after the first "lines " in the window; prefers one that matches
/// a ledger span of `name` (a line may cite two functions with two ranges).
fn best_range_after(window: &str, spans: &[FnSpan], name: &str) -> Option<(usize, usize)> {
    let i = window.find("ines ")? + "ines ".len();
    let ranges = all_ranges(&window[i..]);
    let first = *ranges.first()?;
    Some(
        ranges
            .into_iter()
            .find(|&(a, b)| {
                spans.iter().filter(|x| x.name == name).any(|x| {
                    x.end.is_some_and(|e| a.abs_diff(x.start) <= 25 && b.abs_diff(e) <= 40)
                })
            })
            .unwrap_or(first),
    )
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
            if report[line_start..pos].matches('`').count().is_multiple_of(2) {
                continue;
            }
            let line: &str = report[pos..].lines().next().unwrap_or("");
            let window: String = line.chars().take(300).collect();
            if let Some((a, b)) = best_range_after(&window, spans, &sp.name) {
                let ok = spans.iter().filter(|x| x.name == sp.name).any(|x| {
                    x.end.is_some_and(|e| a.abs_diff(x.start) <= 25 && b.abs_diff(e) <= 40)
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
    // Symbol verification MUST run against the raw evidence blob, not the
    // read_lines map. read_lines exists for line-number tracking
    // (function_spans); using its values as a symbol corpus drops any file
    // body that truncation, chunk overlap, or line-number gaps prevented
    // read_lines from mapping, producing false "symbol not found" rejections
    // on files that were genuinely read in full (see distributed_scheduler.rs
    // vs. remote_synthesize rejection). verification_corpus already strips
    // [HALT lines and preserves FULL FILE CONTENT / CONTENT blocks verbatim.
    let corpus = verification_corpus(local_evidence, "");
    let lines = read_lines(local_evidence);
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

/// One definition line: `(name, is_pub)`. `pub(crate)`/`pub(super)` count as
/// NOT public. Call sites, `use`, and `impl` lines are not definitions.
fn definition_on(t: &str) -> Option<(String, bool)> {
    let mut rest = t;
    let mut is_pub = false;
    if let Some(r) = rest.strip_prefix("pub") {
        if r.starts_with('(') {
            rest = r[r.find(')')? + 1..].trim_start();
        } else if r.starts_with(char::is_whitespace) {
            is_pub = true;
            rest = r.trim_start();
        } else {
            return None;
        }
    }
    loop {
        if let Some(r) = rest.strip_prefix("async ").or_else(|| rest.strip_prefix("unsafe ")) {
            rest = r.trim_start();
        } else if let Some(r) = rest.strip_prefix("const fn ") {
            rest = r.trim_start();
            return definition_name("fn", rest).map(|n| (n, is_pub));
        } else {
            break;
        }
    }
    for kw in ["fn", "struct", "enum", "trait", "type", "const", "static", "mod", "union"] {
        if let Some(r) = rest.strip_prefix(kw) {
            if r.starts_with(char::is_whitespace) {
                let name: String =
                    r.trim_start().chars().take_while(|&c| is_word_char(c)).collect();
                if is_ident(&name) {
                    return Some((name, is_pub));
                }
            }
        }
    }
    if is_pub {
        // public field: `pub name: Type`
        let name: String = rest.chars().take_while(|&c| is_word_char(c)).collect();
        let after = rest[name.len()..].trim_start();
        if is_ident(&name) && after.starts_with(':') && !after.starts_with("::") {
            return Some((name, true));
        }
    }
    None
}

fn definition_name(_kw: &str, rest: &str) -> Option<String> {
    let name: String = rest.chars().take_while(|&c| is_word_char(c)).collect();
    if is_ident(&name) {
        Some(name)
    } else {
        None
    }
}

fn public_definitions(corpus: &str) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for line in corpus.lines() {
        let t = strip_line_number_prefix(line.trim_start()).trim_start();
        if let Some(rest) = t.strip_prefix("pub use ") {
            let body = rest.trim().trim_end_matches(';');
            let items: Vec<&str> = match (body.find('{'), body.rfind('}')) {
                (Some(a), Some(b)) if b > a => body[a + 1..b].split(',').collect(),
                _ => vec![body],
            };
            for it in items {
                let it = it.trim();
                let name = it.rsplit(" as ").next().unwrap_or(it);
                let name = name.rsplit("::").next().unwrap_or(name).trim();
                if is_ident(name) {
                    out.insert(name.to_string());
                }
            }
            continue;
        }
        if let Some((name, true)) = definition_on(t) {
            out.insert(name);
        }
    }
    out
}

/// Splits backticked identifiers in a report into `(grounded, ungrounded)`.
/// A name is grounded only if `corpus` holds a PUBLIC definition of it
/// (`pub fn|struct|enum|trait|type|const|static|mod NAME`, `pub NAME:` field, or
/// a `pub use ... NAME` re-export). Call sites, imports, `impl` headers and
/// `pub(crate)`/private items do not count. Stricter than `unverified_symbols`;
/// use it only when the mission asks for public items.
/// Returns true for tokens that should NOT be treated as claimable
/// public definitions. Audit reports legitimately name standard library
/// types, standard methods, string-literal statuses, FFI/WASM conventions,
/// and canonical entry points -- none of which are `pub fn` / `pub struct`
/// items in the project source. Requiring a project-level `pub` definition
/// for each of them makes the grounding gate unsatisfiable for a
/// comprehensive audit report.
fn is_grounding_skip_token(name: &str) -> bool {
    matches!(
        name,
        // Std types and traits.
        "String" | "Result" | "Option" | "Vec" | "HashMap" | "HashSet"
            | "Arc" | "Mutex" | "RwLock" | "Handle" | "JoinHandle"
            | "Drop" | "Iterator" | "Clone" | "Debug" | "Default"
            | "Send" | "Sync" | "Any" | "Box" | "Duration" | "Instant"
        // Std methods and combinators.
            | "unwrap" | "expect" | "clone" | "to_string" | "from_str" | "parse"
            | "spawn" | "spawn_blocking" | "try_current" | "exit" | "drop"
            | "abort" | "panic" | "flush" | "read" | "write"
        // Entry points, FFI, libc, WASM conventions.
            | "main" | "_start" | "start_server" | "init"
            | "flock" | "setuid" | "setgid" | "getuid" | "getgid"
        // Status literal vocabulary and evidence markers.
            | "Active" | "Aborted" | "Completed" | "Pending" | "Failed" | "Running"
            | "FRACTURE" | "PERSISTENCE" | "INCOMPLETE" | "COMPLETE"
            | "READ_OK" | "WRITE_OK" | "CMD_OK" | "CMD_FAILED"
            | "SEARCH_CODE_OK" | "SEARCH_CHECKED_OK" | "SEARCH_MEMORY_OK"
            | "READ_CHUNK_OK" | "READ_FUNCTION_OK" | "PATCH_FUNCTION_OK"
            | "PATCH_IDEMPOTENT_OK" | "DIFF_PATCH_OK" | "REPLACE_LINES_OK"
            | "HALT" | "DAG_LINEAGE_OK" | "ANALYSIS_OK" | "TIMEOUT"
        // Report section labels.
            | "Scope" | "Summary" | "Evidence" | "Conclusion"
    ) || name.starts_with('_')
}

pub fn check_pub_claims(report: &str, corpus: &str) -> (Vec<String>, Vec<String>) {
    let public = public_definitions(corpus);
    let (mut ok, mut bad): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for span in report.split('`').skip(1).step_by(2) {
        let span = span.trim().trim_end_matches("()");
        let claimed = match claimed_name(span) {
            Some(n) => n,
            None => continue,
        };
        let name = claimed.as_str();
        if SKIP.contains(&name) || is_grounding_skip_token(name) {
            continue;
        }
        let bucket = if public.contains(name) { &mut ok } else { &mut bad };
        if !bucket.iter().any(|b| b == name) {
            bucket.push(name.to_string());
        }
    }
    (ok, bad)
}

/// Backticked public-item claims in `report` that are not verified public in `corpus`.
#[cfg(test)]
fn unverified_pub_claims(report: &str, corpus: &str) -> Vec<String> {
    check_pub_claims(report, corpus).1
}

/// Single-file `search_code` hits: `13:pub struct X {` or context `14-    body`.
fn strip_line_number_prefix(line: &str) -> &str {
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return line;
    }
    match line[digits..].chars().next() {
        Some(':') | Some('-') => &line[digits + 1..],
        _ => line,
    }
}

/// Bare ident (`foo`, `Type::foo`) or keyword-led public claim (`pub fn foo`,
/// `pub async fn foo`, `pub struct Foo`). Paths, `pub use`, `pub(crate)`, prose → None.
fn claimed_name(span: &str) -> Option<String> {
    let bare = span.rsplit("::").next().unwrap_or("");
    if is_ident(bare) {
        return Some(bare.to_string());
    }
    let mut toks = span.split_whitespace();
    if toks.next() != Some("pub") {
        return None;
    }
    let mut t = toks.next()?;
    while t == "async" || t == "unsafe" || (t == "const" && toks.clone().next() == Some("fn")) {
        t = toks.next()?;
    }
    if !["fn", "struct", "enum", "trait", "type", "const", "static", "mod", "union"].contains(&t) {
        return None;
    }
    let name: String = toks.next()?.chars().take_while(|&c| is_word_char(c)).collect();
    if is_ident(&name) {
        Some(name)
    } else {
        None
    }
}

#[cfg(test)]
mod claim_form_tests {
    use super::*;

    const SEARCH: &str = r#"[SEARCH_CODE_OK] Path: crates/runtime_core/src/executor.rs | Query: 'pub '
MATCHES:
11-
12-#[derive(Debug, PartialEq, Clone)]
13:pub enum FsmState {
57:pub fn sanitize(path: &str) -> Result<PathBuf> {
276-impl RoboticExecutor {
277:    pub fn new(api_key: String) -> Self {
289:    pub async fn execute_motor_command(
33:fn canonicalize_or_parent(p: &Path) -> Result<PathBuf> {
40:pub const fn make() -> u8 {
"#;

    #[test]
    fn keyword_led_spans_are_checked_against_single_file_search_hits() {
        let report = "`pub enum FsmState` `pub fn sanitize` `pub async fn execute_motor_command` `pub fn new` `pub const fn make` `pub fn canonicalize_or_parent` `pub fn invented`";
        let (ok, bad) = check_pub_claims(report, SEARCH);
        assert_eq!(ok, vec!["FsmState", "sanitize", "execute_motor_command", "new", "make"]);
        assert_eq!(bad, vec!["canonicalize_or_parent", "invented"]);
    }

    #[test]
    fn restricted_use_and_path_spans_claim_nothing() {
        let (ok, bad) = check_pub_claims(
            "`pub(crate) fn hidden` `pub use a::B;` `src/x.rs` `prose words`",
            SEARCH,
        );
        assert!(ok.is_empty() && bad.is_empty());
    }

    #[test]
    fn line_number_prefix_stripping() {
        assert_eq!(strip_line_number_prefix("13:pub x"), "pub x");
        assert_eq!(strip_line_number_prefix("14-    body"), "    body");
        assert_eq!(strip_line_number_prefix("11-"), "");
        assert_eq!(strip_line_number_prefix("84"), "84");
        assert_eq!(strip_line_number_prefix("--"), "--");
        assert_eq!(strip_line_number_prefix("pub fn z()"), "pub fn z()");
    }
}

/// Report-style artifacts only; source files are never gated here.
pub fn is_report_artifact(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".txt") || p.ends_with(".md")
}

/// Strip lineage sections from a composite mission so that
/// `wants_pub_items` only inspects the node's own objective and the
/// root directive — not evidence inherited from sibling/parent nodes
/// (which frequently contains `pub struct` propositions from upstream
/// `[PROPOSITION] ... Defined symbols: pub struct X` lines).
fn strip_lineage(text: &str) -> &str {
    let cut = [
        text.find("[DEPENDENCY LINEAGE FROM"),
        text.find("[DAG LINEAGE"),
        text.find("LINEAGE CONTEXT:"),
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(text.len());
    &text[..cut]
}

/// True when the mission asks for public items (so the strict check applies).
pub fn wants_pub_items(objective: &str) -> bool {
    let o = strip_lineage(objective).to_ascii_lowercase();
    ["pub fn", "pub struct", "pub enum", "pub trait", "public item", "public api"]
        .iter()
        .any(|k| o.contains(k))
}

#[cfg(test)]
mod pub_claim_tests {
    use super::*;

    const EVIDENCE: &str = r#"[READ_OK] crates/runtime_core/src/epistemic_engine.rs
FULL FILE CONTENT:
use crate::json_parser::sanitize_json_escapes;
pub struct CriticResponse {
    pub critique: String,
    pub confidence_modifier: f32,
}
pub struct EpistemicEngine;
impl EpistemicEngine {
    pub async fn safe_api_call(
        client: &Client,
    ) -> Result<String> {
    pub(crate) fn strip_thought_tags(s: &str) -> String {
    pub(crate) fn compress_preserving_reads(evidence: &str) -> String {
        let read_targets =
            crate::completion_engine::extract_all_named_targets(scoped_problem, "READ_OK:");
        if crate::completion_engine::any_word_unnegated(&problem_lower, &["search"]) {
fn canonicalize_or_parent(p: &Path) -> Result<PathBuf> {
pub fn sanitize(path: &str) -> Result<PathBuf> {
pub enum FsmState {
use crate::cortical_loop::CorticalLoop;
use crate::cognition::epistemic_session::EpistemicSession;
impl CorticalLoop {
    pub(crate) fn apply_ast_unaddressable_recovery(
pub use client::ApiClient;
pub use config::{NexusConfig, Other as Renamed};
pub mod agent_protocol;
pub(crate) mod cortical;
"#;

    #[test]
    fn flags_every_bad_name_from_the_live_report() {
        let report = "`CriticResponse` `EpistemicEngine` `safe_api_call` `strip_thought_tags` `compress_preserving_reads` `canonicalize_or_parent` `any_word_unnegated` `extract_all_named_targets` `create_search_memory_graph` `CorticalLoop` `EpistemicSession` `apply_ast_unaddressable_recovery`";
        let mut bad = unverified_pub_claims(report, EVIDENCE);
        bad.sort();
        let mut want = vec![
            "strip_thought_tags",
            "compress_preserving_reads",
            "canonicalize_or_parent",
            "any_word_unnegated",
            "extract_all_named_targets",
            "create_search_memory_graph",
            "CorticalLoop",
            "EpistemicSession",
            "apply_ast_unaddressable_recovery",
        ];
        want.sort();
        assert_eq!(bad, want);
    }

    #[test]
    fn accepts_real_public_items_fields_reexports_and_modules() {
        let report = "`CriticResponse`, `critique`, `confidence_modifier`, `EpistemicEngine::safe_api_call()`, `sanitize`, `FsmState`, `ApiClient`, `NexusConfig`, `Renamed`, `agent_protocol`";
        assert!(unverified_pub_claims(report, EVIDENCE).is_empty());
    }

    #[test]
    fn restricted_modules_do_not_count_as_public() {
        assert_eq!(unverified_pub_claims("`cortical`", EVIDENCE), vec!["cortical"]);
    }

    #[test]
    fn spans_with_spaces_paths_or_fences_are_ignored() {
        let report = "`pub use client::ApiClient;` `src/lib.rs` ```sh\nfind . | wc -l\n```";
        assert!(unverified_pub_claims(report, EVIDENCE).is_empty());
    }

    #[test]
    fn artifact_and_mission_detection() {
        assert!(is_report_artifact("analysis_report.txt"));
        assert!(!is_report_artifact("crates/runtime_core/src/lib.rs"));
        assert!(wants_pub_items("list 3-5 `pub fn` or pub struct names"));
        assert!(!wants_pub_items("summarize the module structure"));
    }

    #[test]
    fn placeholder_report_has_no_grounded_items() {
        let (ok, bad) = check_pub_claims("placeholder\n", EVIDENCE);
        assert!(ok.is_empty() && bad.is_empty());
        let (ok, bad) = check_pub_claims("`sanitize` and `CorticalLoop`", EVIDENCE);
        assert_eq!(ok, vec!["sanitize"]);
        assert_eq!(bad, vec!["CorticalLoop"]);
    }
}

#[cfg(test)]
mod halt_claims_keyword_tests {
    use super::unverified_symbols;

    #[test]
    fn absence_claims_citing_keywords_are_not_unverified() {
        let corpus = "pub fn add(a: u8) -> u8 { a }";
        let report = "No `unsafe`, `static`, `const` or `impl` blocks; `add` is the only fn.";
        assert!(unverified_symbols(report, corpus).is_empty());
    }
}

#[cfg(test)]
mod two_range_tests {
    use super::*;
    #[test]
    fn two_ranges_on_one_line_accept_the_matching_one() {
        let mut m: std::collections::BTreeMap<usize, String> =
            (1..=30).map(|n| (n, "    x;".to_string())).collect();
        m.insert(1, "fn wrapper() {".to_string());
        m.insert(3, "}".to_string());
        m.insert(5, "fn big() {".to_string());
        m.insert(30, "}".to_string());
        let sp = function_spans(&m);
        let r = "through `wrapper` / `big` (lines 1-3 and 5-30)";
        assert!(range_defects(r, &sp, 1).is_empty(), "{:?}", range_defects(r, &sp, 1));
        assert_eq!(range_defects("`big` (lines 100-180)", &sp, 1).len(), 1);
    }
}

#[cfg(test)]
mod absence_tests {
    use super::*;
    #[test]
    fn absence_claims_pass_invented_claims_fail() {
        let corpus = "fn real() {}";
        assert!(unverified_symbols("(c) No `active_graph` symbol exists.", corpus).is_empty());
        assert!(
            unverified_symbols("The `active_graph` identifier does not appear.", corpus).is_empty()
        );
        assert_eq!(unverified_symbols("`invented_fn` returns 3", corpus), vec!["invented_fn"]);
    }
}

#[cfg(test)]
mod wants_pub_items_dry_run {
    use super::*;

    const NODE4_OBJECTIVE: &str = "Write a comprehensive report named `full_system_audit.md` containing: - Orphaned files and dead code found via cargo warnings. - Exact locations of stubs, placeholders, and technical debt. - Identification of any duplicated logic across crates. - Concrete architectural advice for advancing the system to live, unmonitored production runs.";

    const ROOT_DIRECTIVE: &str = "OBJECTIVE: Perform a full system analysis of the bins/ apps/ and crates/ directories. Audit for orphaned files, stubs, duplicate logic, and production readiness. Step 1: Execute exactly `cargo check --workspace`. Step 2: Search the codebase for \"todo!\" and \"unimplemented!\" macros. Step 3: Read bins/ide_backend/src/main.rs and crates/runtime_core/src/task_manager.rs. Step 4: Write a comprehensive report named `full_system_audit.md`.";

    const NODE3_LINEAGE: &str = "---\n[DEPENDENCY LINEAGE FROM task_3]:\n[DAG NODE COMPLETE] Objective: Read bins/ide_backend/src/main.rs and crates/runtime_core/src/task_manager.rs\nBelief: 0.95\nEvidence:\n- [Supports] [READ_OK] crates/runtime_core/src/task_manager.rs\n[PROPOSITION] crates/runtime_core/src/task_manager.rs -> Defined symbols: pub struct TaskContext\n- [Supports] [HALT] READ_OK criteria are satisfied...\n---\n";

    fn composite_mission() -> String {
        format!(
            "OBJECTIVE: {}\nASSIGNED ROLE: CodeAnalyzer\n\n\
             MANDATORY NODE SUCCESS CRITERIA:\n- WritePathAcquired {{ path: \"full_system_audit.md\" }}\n\n\
             NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced before this node can be considered complete.\n\n\
             ROOT DIRECTIVE CONTEXT:\n{}\n\n\
             LINEAGE CONTEXT:\n{}",
            NODE4_OBJECTIVE, ROOT_DIRECTIVE, NODE3_LINEAGE
        )
    }

    #[test]
    fn components_are_not_pub_item_requests() {
        assert!(
            !wants_pub_items(NODE4_OBJECTIVE),
            "node 4's own objective does NOT ask for public API items"
        );
        assert!(
            !wants_pub_items(ROOT_DIRECTIVE),
            "the root directive does NOT ask for public API items"
        );
        assert!(
            NODE3_LINEAGE.contains("pub struct"),
            "sanity: lineage DOES contain 'pub struct' from upstream evidence"
        );
    }

    #[test]
    fn composite_mission_no_longer_fires_the_gate() {
        let composite = composite_mission();
        assert!(
            !wants_pub_items(&composite),
            "FIX CONFIRMED: composite mission no longer triggers wants_pub_items"
        );
    }

    #[test]
    fn genuine_pub_item_missions_still_fire() {
        assert!(
            wants_pub_items("List the pub fn items in src/lib.rs"),
            "REGRESSION: real pub-fn missions must still trigger the gate"
        );
        assert!(
            wants_pub_items("Enumerate the public API of nexus_runtime_core"),
            "REGRESSION: 'public API' missions must still trigger"
        );
        assert!(
            wants_pub_items("Output every pub struct Name in the crate"),
            "REGRESSION: 'pub struct' missions must still trigger"
        );
    }

    #[test]
    fn strip_lineage_is_what_makes_it_work() {
        let composite = composite_mission();
        let stripped_len = strip_lineage(&composite).len();
        assert!(
            stripped_len < composite.len(),
            "strip_lineage must shorten a lineage-bearing mission"
        );
        let stripped = &composite[..stripped_len];
        assert!(
            !stripped.to_ascii_lowercase().contains("pub struct"),
            "after stripping, the string no longer contains 'pub struct'"
        );
    }

    #[test]
    fn strip_lineage_picks_the_earliest_marker() {
        let text =
            "OBJECTIVE: foo\n\nLINEAGE CONTEXT:\n[DEPENDENCY LINEAGE FROM t]:\npub struct Bar";
        let cut = strip_lineage(text);
        assert!(!cut.contains("pub struct"));
        assert!(cut.contains("OBJECTIVE: foo"));
    }
}
