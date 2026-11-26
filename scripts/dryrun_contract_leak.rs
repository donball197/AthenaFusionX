//! Surgical dry run: contract leak + identifier gate + forced-read substitution.
//!
//! Standalone (std only). Does NOT touch the repo or the cargo build:
//!     rustc -O scripts/dryrun_contract_leak.rs -o /tmp/dryrun_leak && /tmp/dryrun_leak
//!
//! Each section holds the CURRENT logic (copied from the sources) next to
//! the FIXED logic, runs both on the exact missions from the failed vision run,
//! and checks two things: the bug reproduces on CURRENT, and the fix holds on FIXED.
//! Exit code 0 only if every expectation holds.
//!
//! Sources mirrored:
//!   completion_engine.rs  extract_required_artifact_paths / MissionScopes::parse /
//!                         infer_contract -> merge_prose_deliverables
//!   cortical/python_conformance.rs  step 2 (snake_case identifiers named in the objective)
//!   cortical/recovery.rs + action_gate.rs  forced-read marker semantics
//! Like report_readonly_dryrun.rs, this is an approximation of the real code paths.
//! The authoritative checks are the cargo regression tests; this shows the delta fast.

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Section 1: contract leak (completion_engine.rs)
// ---------------------------------------------------------------------------

const WRITE_WORDS: [&str; 8] =
    ["write", "save", "export", "synthesize", "create", "implement", "integrate", "add"];
const READ_WORDS: [&str; 4] = ["read", "reading", "inspect", "inspecting"];

/// `nearest_verb == false` is the CURRENT code (any write verb within the clause,
/// capped at 30 tokens, counts). `true` is FIX B (the verb closest to the path decides).
fn extract_artifacts(objective: &str, nearest_verb: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let exts = ["md", "json", "rs", "toml", "yaml", "yml", "txt", "py"];
    let tokens: Vec<&str> = objective.split_whitespace().collect();

    for (i, tok) in tokens.iter().enumerate() {
        let trimmed = tok
            .trim_matches(|c: char| {
                !c.is_alphanumeric() && c != '.' && c != '/' && c != '_' && c != '-'
            })
            .trim_end_matches('.');
        let was_criterion_marked =
            trimmed.starts_with("WRITE_OK:") || trimmed.starts_with("DIFF_PATCH_OK:");
        let clean = trimmed
            .strip_prefix("WRITE_OK:")
            .or_else(|| trimmed.strip_prefix("DIFF_PATCH_OK:"))
            .unwrap_or(trimmed);

        if exts.iter().any(|ext| clean.ends_with(&format!(".{}", ext)))
            && (clean.contains('/') || clean.contains('.'))
        {
            let min_start = i.saturating_sub(30);
            let mut start = i;
            while start > min_start && !tokens[start - 1].ends_with(',') {
                start -= 1;
            }
            let word = |w: &str| {
                w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
            };
            let is_write_tgt = was_criterion_marked
                || if nearest_verb {
                    tokens[start..i]
                        .iter()
                        .rev()
                        .find_map(|w| {
                            let wl = word(w);
                            if WRITE_WORDS.contains(&wl.as_str()) {
                                Some(true)
                            } else if READ_WORDS.contains(&wl.as_str()) {
                                Some(false)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(false)
                } else {
                    tokens[start..i].iter().any(|w| WRITE_WORDS.contains(&word(w).as_str()))
                };
            if is_write_tgt && !out.iter().any(|a| a == clean) {
                out.push(clean.to_string());
            }
        }
    }
    out
}

struct Scopes<'a> {
    objective: &'a str,
}

/// Same header handling as MissionScopes::parse (objective = text before the
/// criteria header, or before the root header when no criteria header exists).
fn parse_scopes(raw: &str) -> Scopes<'_> {
    const CRITERIA_HEADERS: [&str; 4] =
        ["MANDATORY NODE SUCCESS CRITERIA:", "CRITERIA:", "SUCCESS CRITERIA:", "NODE SUCCESS CRITERIA:"];
    let criteria_start = CRITERIA_HEADERS.iter().filter_map(|h| raw.find(h)).min();
    let root_start = raw.find("ROOT DIRECTIVE CONTEXT:");
    if criteria_start.is_none() && root_start.is_none() {
        return Scopes { objective: raw };
    }
    let end = criteria_start.or(root_start).unwrap_or(raw.len());
    Scopes { objective: &raw[..end] }
}

/// WRITE_OK: lines in the criteria block become artifacts in infer_contract_raw
/// (declared_criteria_contract early return). Prose merging happens afterwards.
fn declared_write_ok(raw: &str) -> Vec<String> {
    let crit = raw
        .split("MANDATORY NODE SUCCESS CRITERIA:")
        .nth(1)
        .map(|s| s.split("NODE COMPLETION RULE").next().unwrap_or(s))
        .unwrap_or("");
    crit.lines()
        .filter_map(|l| l.trim().strip_prefix("WRITE_OK:").map(|p| p.trim().to_string()))
        .collect()
}

/// CURRENT: infer_contract -> merge_prose_deliverables(&mut contract, RAW mission).
/// FIXED (A + B): merge over scopes.objective only, with the nearest-verb rule.
fn contract_artifacts(raw: &str, fixed: bool) -> Vec<String> {
    let mut arts = declared_write_ok(raw);
    let merge_text = if fixed { parse_scopes(raw).objective } else { raw };
    for p in extract_artifacts(merge_text, fixed) {
        if !arts.contains(&p) {
            arts.push(p);
        }
    }
    arts
}

const USER_PROMPT: &str = "You are working in /root/AthenaFusionX/glm-5.3-flash-from-scratch.\n\nWrite unit tests for the untested vision and multimodal components.\n\n1. Read these two files completely:\n   - glm53_flash/vision.py\n   - tests/test_vision.py\n\n2. Create a new test file tests/test_vision_unit.py that provides independent numerical / structural unit tests for every exported vision/multimodal symbol that currently has zero unit coverage.\n\n3. Prefer independent reference implementations (explicit loops / matmul) where the component has non-trivial math, matching the style of tests/test_model_correctness.py.\n\n4. After writing the file, run:\n   source .venv/bin/activate && python -m pytest tests/test_vision_unit.py -v\n\nReport success only when the new test file exists and the pytest command passes.";

fn compose(objective: &str, role: &str, criteria: &str) -> String {
    format!(
        "OBJECTIVE: {objective}\nASSIGNED ROLE: {role}\n\nMANDATORY NODE SUCCESS CRITERIA:\n{criteria}\n\nNODE COMPLETION RULE: Every mandatory criterion must be physically evidenced before this node can be considered complete.\n\nROOT DIRECTIVE CONTEXT:\n{USER_PROMPT}\n\nLINEAGE CONTEXT:\n"
    )
}

const NODE1_OBJ: &str = "Read glm53_flash/vision.py and tests/test_vision.py completely to inspect exported vision/multimodal symbols and existing test patterns.";
const NODE2_OBJ: &str = "Create a new test file tests/test_vision_unit.py that provides independent numerical and structural unit tests for every exported vision/multimodal symbol that currently has zero unit coverage, covering MiniGLMVisionEncoder, MiniVisionConfig, PatchImageEncoder, DirectPatchImageEncoder, RGBPatchEmbedding, SpatialMerger2x2, VisionTransformerBlock, MultimodalProjector, VisionLanguageModel, DirectPatchVisionLanguageModel, ImageTokenIds, answer_only_labels, and build_multimodal_input_ids using independent reference implementations matching tests/test_model_correctness.py style.";
const NODE3_OBJ: &str = "Run the new unit tests by executing: source .venv/bin/activate && python -m pytest tests/test_vision_unit.py -v";

// ---------------------------------------------------------------------------
// Section 2: identifier gate (cortical/python_conformance.rs, step 2)
// ---------------------------------------------------------------------------

const TOOL_NAMES: [&str; 8] = [
    "read_file", "write_file", "search_code", "execute_command", "patch_function",
    "replace_lines", "diff_patch", "read_file_chunk",
];

fn required_idents(objective: &str, path: &str, fixed: bool) -> Vec<String> {
    let path_stem =
        std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();

    // FIX: components of file paths named in the objective are locations, not identifiers.
    let mut path_tokens: HashSet<String> = HashSet::new();
    if fixed {
        for word in objective.split_whitespace() {
            let w = word.trim_matches(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-'))
            });
            let looks_like_path = w.contains('/')
                || w.rsplit_once('.').is_some_and(|(stem, ext)| {
                    !stem.is_empty()
                        && (1..=5).contains(&ext.len())
                        && ext.chars().all(|c| c.is_ascii_alphanumeric())
                });
            if looks_like_path {
                for part in w.split(|c: char| matches!(c, '/' | '.' | '-')) {
                    if !part.is_empty() {
                        path_tokens.insert(part.to_string());
                    }
                }
            }
        }
    }

    let mut idents: Vec<String> = Vec::new();
    for tok in objective.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        let tok = tok.trim_matches('_');
        if tok.len() < 3 || !tok.contains('_') {
            continue;
        }
        if tok.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            continue;
        }
        if tok == path_stem || path_tokens.contains(tok) {
            continue;
        }
        if TOOL_NAMES.contains(&tok) {
            continue;
        }
        if !idents.iter().any(|i| i == tok) {
            idents.push(tok.to_string());
        }
    }
    idents
}

fn missing_idents(objective: &str, path: &str, content: &str, fixed: bool) -> Vec<String> {
    required_idents(objective, path, fixed)
        .into_iter()
        .filter(|t| !content.contains(t.as_str()))
        .collect()
}

// ---------------------------------------------------------------------------
// Section 3: forced-read substitution (recovery.rs + action_gate.rs semantics)
// ---------------------------------------------------------------------------

fn read_met(evidence: &str, path: &str) -> bool {
    evidence.contains(&format!("[READ_OK] {path}"))
        || evidence.contains(&format!("[READ_CHUNK_OK] File: {path} "))
}

/// FIX: replace a lone read_file of a different path with the forced, unmet read.
fn substitute_forced_read(evidence: &str, proposed: &str) -> Option<String> {
    let last = evidence
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with("[CONTRACT_FIRST_FORCED]"))?
        .trim();
    let expected = last.strip_prefix("[CONTRACT_FIRST_FORCED] read_file:")?.trim();
    if expected.is_empty() || expected == proposed {
        return None;
    }
    if read_met(evidence, expected)
        || evidence.lines().filter(|l| l.trim() == last).count() >= 3
    {
        return None;
    }
    Some(expected.to_string())
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Filtered,
    Runs(String),
}

/// Pipeline model from the Mission 01 log: proposal -> anti-loop (drops anything
/// already attempted) -> execute. CURRENT has no substitution step; FIXED runs it first.
fn pipeline(evidence: &str, proposed: &str, attempted: &[&str], fixed: bool) -> Outcome {
    let action = if fixed {
        substitute_forced_read(evidence, proposed).unwrap_or_else(|| proposed.to_string())
    } else {
        proposed.to_string()
    };
    if attempted.contains(&action.as_str()) {
        Outcome::Filtered
    } else {
        Outcome::Runs(action)
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Report {
    total: usize,
    failed: usize,
}

impl Report {
    fn check(&mut self, label: &str, ok: bool, detail: String) {
        self.total += 1;
        if !ok {
            self.failed += 1;
        }
        println!("  [{}] {label}\n        {detail}", if ok { "ok  " } else { "FAIL" });
    }
}

fn main() {
    let mut r = Report { total: 0, failed: 0 };
    let leak: Vec<String> = ["glm53_flash/vision.py", "tests/test_vision.py", "tests/test_vision_unit.py"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    // == Section 1: contract leak ==
    println!("== 1. CONTRACT LEAK: required_artifact_paths per DAG node ==");
    let n1 = compose(NODE1_OBJ, "CodeAnalyzer", "READ_OK:glm53_flash/vision.py\nREAD_OK:tests/test_vision.py");
    let n2 = compose(NODE2_OBJ, "Coder", "WRITE_OK:tests/test_vision_unit.py");
    let n3 = compose(NODE3_OBJ, "Tester", "CMD_OK");
    let cases: [(&str, &String, Vec<String>); 3] = [
        ("node 1 (read-only recon)", &n1, vec![]),
        ("node 2 (writer)", &n2, vec!["tests/test_vision_unit.py".to_string()]),
        ("node 3 (runs pytest)", &n3, vec![]),
    ];
    for (name, mission, want) in cases.iter() {
        let cur = contract_artifacts(mission, false);
        let fix = contract_artifacts(mission, true);
        println!("{name}\n    current: {cur:?}\n    fixed  : {fix:?}");
        r.check(&format!("{name}: fixed matches expected"), fix == *want, format!("want {want:?}"));
    }
    let cur1 = contract_artifacts(&n1, false);
    r.check(
        "node 1: CURRENT leaks the 3 paths seen in the failed run",
        leak.iter().all(|p| cur1.contains(p)),
        format!("current = {cur1:?}"),
    );
    let cur3 = contract_artifacts(&n3, false);
    r.check(
        "node 3: CURRENT also leaks (tester node would demand a write)",
        cur3.contains(&"tests/test_vision_unit.py".to_string()),
        format!("current = {cur3:?}"),
    );
    // Fix B alone must not break ordinary "read X then write Y" prose.
    let prose = "Read src/a.rs and then write the summary to report.md.";
    let b = extract_artifacts(prose, true);
    r.check(
        "fix B keeps a real write target, drops the read target",
        b == vec!["report.md".to_string()],
        format!("got {b:?}"),
    );

    // == Section 2: identifier gate ==
    println!("\n== 2. IDENTIFIER GATE: python_conformance.rs step 2 ==");
    let m1c = missing_idents(NODE1_OBJ, "glm53_flash/vision.py", "import torch.nn as nn", false);
    let m1f = missing_idents(NODE1_OBJ, "glm53_flash/vision.py", "import torch.nn as nn", true);
    println!("node 1 objective, stub content\n    current: {m1c:?}\n    fixed  : {m1f:?}");
    r.check(
        "CURRENT reproduces run-3 defects (glm53_flash, test_vision)",
        m1c.contains(&"glm53_flash".to_string()) && m1c.contains(&"test_vision".to_string()),
        format!("{m1c:?}"),
    );
    r.check("FIXED: no path-derived identifiers required", m1f.is_empty(), format!("{m1f:?}"));

    let stub = "import pytest\n";
    let m2c = missing_idents(NODE2_OBJ, "tests/test_vision_unit.py", stub, false);
    let m2f = missing_idents(NODE2_OBJ, "tests/test_vision_unit.py", stub, true);
    println!("node 2 objective, stub content\n    current: {m2c:?}\n    fixed  : {m2f:?}");
    r.check(
        "CURRENT reproduces run-2 defects (incl. test_model_correctness)",
        m2c.contains(&"test_model_correctness".to_string())
            && m2c.contains(&"answer_only_labels".to_string()),
        format!("{m2c:?}"),
    );
    r.check(
        "FIXED: still demands the two real identifiers, nothing else",
        m2f == vec!["answer_only_labels".to_string(), "build_multimodal_input_ids".to_string()],
        format!("{m2f:?}"),
    );
    let full = "def test_a():\n    answer_only_labels\n    build_multimodal_input_ids\n";
    let m2ok = missing_idents(NODE2_OBJ, "tests/test_vision_unit.py", full, true);
    r.check("FIXED: conforming file has zero defects", m2ok.is_empty(), format!("{m2ok:?}"));
    let dotted = missing_idents(
        "Write monitor.py using shutil.disk_usage to report usage.",
        "monitor.py",
        "import os\n",
        true,
    );
    r.check(
        "FIXED: dotted call `shutil.disk_usage` is not mistaken for a path",
        dotted == vec!["disk_usage".to_string()],
        format!("{dotted:?}"),
    );

    // == Section 3: forced read ==
    println!("\n== 3. FORCED READ: Mission 01 turns 53-63 ==");
    let evidence = "[READ_OK] crates/runtime_core/src/completion_engine.rs\n[READ_OK] crates/runtime_core/src/epistemic_engine.rs\n[READ_OK] crates/runtime_core/src/anti_loop.rs\n[READ_OK] crates/runtime_core/src/cortical_loop/critic.rs\n[RECON_REQUESTED] read_file:crates/runtime_core/src/cortical_loop/strategize.rs (strategy forced; NOT read evidence)\n[CONTRACT_FIRST_FORCED] read_file:crates/runtime_core/src/cortical_loop/strategize.rs\n";
    let attempted = ["crates/runtime_core/src/epistemic_engine.rs"];
    let stale = "crates/runtime_core/src/epistemic_engine.rs";
    let want = "crates/runtime_core/src/cortical_loop/strategize.rs";
    let cur = pipeline(evidence, stale, &attempted, false);
    let fix = pipeline(evidence, stale, &attempted, true);
    println!("strategist re-proposes read of {stale}\n    current: {cur:?}\n    fixed  : {fix:?}");
    r.check("CURRENT: stale read is filtered, the forced read never runs", cur == Outcome::Filtered, format!("{cur:?}"));
    r.check("FIXED: proposal is rewritten to the mandated read", fix == Outcome::Runs(want.to_string()), format!("{fix:?}"));
    r.check(
        "matching path is left alone",
        pipeline(evidence, want, &attempted, true) == Outcome::Runs(want.to_string()),
        String::new(),
    );
    let ev_done = format!("{evidence}[READ_OK] {want}\n");
    r.check(
        "released once the forced read is satisfied",
        pipeline(&ev_done, "crates/other.rs", &attempted, true) == Outcome::Runs("crates/other.rs".to_string()),
        String::new(),
    );
    let ev3 = format!("{evidence}[CONTRACT_FIRST_FORCED] read_file:{want}\n[CONTRACT_FIRST_FORCED] read_file:{want}\n");
    r.check(
        "released after 3 identical forcings (no infinite pin)",
        pipeline(&ev3, "crates/other.rs", &attempted, true) == Outcome::Runs("crates/other.rs".to_string()),
        String::new(),
    );

    println!("\n== RESULT: {}/{} expectations hold ==", r.total - r.failed, r.total);
    if r.failed > 0 {
        std::process::exit(1);
    }
}
