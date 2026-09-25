//! Language-neutral Tree-sitter diff engine.
//!
//! The model proposes a replacement body.
//! This module determines the exact AST body boundary, performs the splice,
//! reparses the complete resulting source, and returns an auditable diff.
//!
//! No filesystem mutation occurs here.

use crate::polyglot::SupportedLanguage;
use anyhow::{anyhow, Result};
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug, Clone)]
pub struct DiffContext {
    pub language: SupportedLanguage,
    pub symbol: String,
    pub body_start: usize,
    pub body_end: usize,
    pub original_body: String,
}

#[derive(Debug, Clone)]
pub struct DiffResult {
    pub context: DiffContext,
    pub original_source: String,
    pub mutated_source: String,
    pub unified_diff: String,
}

pub struct PolyglotDiffEngine;

impl PolyglotDiffEngine {
    /// Read a source file and locate the exact Tree-sitter body belonging
    /// to `symbol`.
    pub fn analyze_file(path: &Path, symbol: &str) -> Result<DiffContext> {
        let source = std::fs::read_to_string(path)?;
        Self::analyze_source(path, &source, symbol)
    }

    /// Locate a symbol's body using the language selected from its path.
    pub fn analyze_source(path: &Path, source: &str, symbol: &str) -> Result<DiffContext> {
        let language = SupportedLanguage::from_path(path)
            .ok_or_else(|| anyhow!("Unsupported source language: {}", path.display()))?;

        Self::analyze_source_with_language(language, source, symbol)
    }

    /// Locate a symbol's exact body byte range using Tree-sitter.
    pub fn analyze_source_with_language(
        language: SupportedLanguage,
        source: &str,
        symbol: &str,
    ) -> Result<DiffContext> {
        let ts_language = language.tree_sitter_language();

        let mut parser = Parser::new();
        parser
            .set_language(&ts_language)
            .map_err(|e| anyhow!("Failed to configure Tree-sitter: {e}"))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow!("Tree-sitter produced no parse tree"))?;

        if tree.root_node().has_error() {
            return Err(anyhow!("Source does not parse cleanly with Tree-sitter ({language:?})"));
        }

        let query = Query::new(&ts_language, language.symbol_query())
            .map_err(|e| anyhow!("Failed to compile symbol query: {e}"))?;

        let name_capture = query
            .capture_names()
            .iter()
            .position(|name| *name == "name")
            .ok_or_else(|| anyhow!("Symbol query has no `name` capture"))?;

        let body_capture = query.capture_names().iter().position(|name| *name == "body");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());

        let mut candidates = Vec::new();

        for query_match in &mut matches {
            let matching_name =
                query_match.captures.iter().find(|capture| capture.index as usize == name_capture);

            let Some(name_capture_node) = matching_name.map(|capture| capture.node) else {
                continue;
            };

            let Ok(name) = name_capture_node.utf8_text(source.as_bytes()) else {
                continue;
            };

            if name != symbol {
                continue;
            }

            // Some grammars can associate the symbol name and its body
            // directly in the same Tree-sitter query match. Prefer that
            // explicit relationship when available. This is required for
            // variable-bound JavaScript/TypeScript functions and arrows,
            // where the name identifier is not an ancestor of the function.
            if let Some(body_capture) = body_capture {
                if let Some(body_node) = query_match
                    .captures
                    .iter()
                    .find(|capture| capture.index as usize == body_capture)
                    .map(|capture| capture.node)
                {
                    candidates.push(body_node.byte_range());
                    continue;
                }
            }

            // Existing fallback for languages whose queries only capture
            // the symbol name. For C/C++, for example, the name may be
            // inside a declarator. Walk upward until Tree-sitter exposes
            // a `body` field.
            let mut node = Some(name_capture_node);

            while let Some(current) = node {
                if let Some(body) = current.child_by_field_name("body") {
                    candidates.push(body.byte_range());
                    break;
                }

                node = current.parent();
            }
        }

        match candidates.len() {
            0 => Err(anyhow!("Symbol `{symbol}` has no Tree-sitter body in {language:?}")),

            1 => {
                let range = &candidates[0];
                let start = range.start;
                let end = range.end;

                if start >= end
                    || end > source.len()
                    || !source.is_char_boundary(start)
                    || !source.is_char_boundary(end)
                {
                    return Err(anyhow!(
                        "Tree-sitter returned an invalid body byte range for `{symbol}`"
                    ));
                }

                Ok(DiffContext {
                    language,
                    symbol: symbol.to_string(),
                    body_start: start,
                    body_end: end,
                    original_body: source[start..end].to_string(),
                })
            }

            count => {
                Err(anyhow!("Symbol `{symbol}` is ambiguous: Tree-sitter found {count} bodies"))
            }
        }
    }

    /// Apply a proposed replacement body without touching the filesystem.
    ///
    /// The proposal is spliced into the exact Tree-sitter body range and the
    /// entire resulting source is reparsed before it can become a DiffResult.
    pub fn apply(
        context: &DiffContext,
        original_source: &str,
        proposed_body: &str,
    ) -> Result<DiffResult> {
        if context.body_start > context.body_end
            || context.body_end > original_source.len()
            || !original_source.is_char_boundary(context.body_start)
            || !original_source.is_char_boundary(context.body_end)
        {
            return Err(anyhow!("Diff context contains an invalid UTF-8 byte range"));
        }

        let actual_body = &original_source[context.body_start..context.body_end];

        if actual_body != context.original_body {
            return Err(anyhow!(
                "Diff context does not match the supplied original source for `{}`",
                context.symbol
            ));
        }

        let mut mutated = String::new();

        mutated.push_str(&original_source[..context.body_start]);
        mutated.push_str(proposed_body);
        mutated.push_str(&original_source[context.body_end..]);

        if !context.language.parses_cleanly(&mutated) {
            return Err(anyhow!(
                "Tree-sitter rejected the proposed {:?} mutation for `{}`",
                context.language,
                context.symbol
            ));
        }

        let unified_diff = Self::unified_diff(original_source, &mutated, &context.symbol);

        Ok(DiffResult {
            context: context.clone(),
            original_source: original_source.to_string(),
            mutated_source: mutated,
            unified_diff,
        })
    }

    /// Produce a deterministic, human-readable unified-style diff.
    ///
    /// The first implementation deliberately focuses on the exact changed
    /// symbol rather than introducing a third-party diff dependency.
    pub fn unified_diff(original: &str, mutated: &str, symbol: &str) -> String {
        if original == mutated {
            return String::new();
        }

        let old_lines: Vec<&str> = original.lines().collect();
        let new_lines: Vec<&str> = mutated.lines().collect();

        let mut first = 0usize;
        while first < old_lines.len()
            && first < new_lines.len()
            && old_lines[first] == new_lines[first]
        {
            first += 1;
        }

        let mut old_end = old_lines.len();
        let mut new_end = new_lines.len();

        while old_end > first && new_end > first && old_lines[old_end - 1] == new_lines[new_end - 1]
        {
            old_end -= 1;
            new_end -= 1;
        }

        let context = 3usize;

        let old_start = first.saturating_sub(context);
        let new_start = first.saturating_sub(context);

        let old_stop = (old_end + context).min(old_lines.len());
        let new_stop = (new_end + context).min(new_lines.len());

        let old_count = old_stop.saturating_sub(old_start);
        let new_count = new_stop.saturating_sub(new_start);

        let mut diff = String::new();

        diff.push_str(&format!("--- a/{symbol}\n"));
        diff.push_str(&format!("+++ b/{symbol}\n"));
        diff.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_start + 1,
            old_count,
            new_start + 1,
            new_count
        ));

        // Deterministic replacement representation.
        for line in &old_lines[old_start..first] {
            diff.push_str(&format!(" {line}\n"));
        }

        for line in &old_lines[first..old_end] {
            diff.push_str(&format!("-{line}\n"));
        }

        for line in &new_lines[first..new_end] {
            diff.push_str(&format!("+{line}\n"));
        }

        for line in &old_lines[old_end..old_stop] {
            diff.push_str(&format!(" {line}\n"));
        }

        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_function_body_is_located_by_tree_sitter() {
        let source = r#"
fn first() -> i32 {
    1
}

fn target() -> i32 {
    2
}
"#;

        let path = Path::new("example.rs");
        let context = PolyglotDiffEngine::analyze_source(path, source, "target").unwrap();

        assert_eq!(context.language, SupportedLanguage::Rust);
        assert_eq!(context.symbol, "target");
        assert_eq!(context.original_body, "{\n    2\n}");
    }

    #[test]
    fn rust_mutation_is_reparsed_before_acceptance() {
        let source = r#"
fn target() -> i32 {
    2
}
"#;

        let path = Path::new("example.rs");
        let context = PolyglotDiffEngine::analyze_source(path, source, "target").unwrap();

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    42\n}").unwrap();

        assert!(result.mutated_source.contains("42"));
        assert_ne!(result.original_source, result.mutated_source);
        assert!(!result.unified_diff.is_empty());
    }

    #[test]
    fn invalid_result_is_rejected() {
        let source = r#"
fn target() -> i32 {
    2
}
"#;

        let path = Path::new("example.rs");
        let context = PolyglotDiffEngine::analyze_source(path, source, "target").unwrap();

        let result = PolyglotDiffEngine::apply(&context, source, "{ invalid !!! }");

        assert!(result.is_err());
    }

    #[test]
    fn nine_language_target_boundary_and_mutation_smoke_test() {
        let cases = [
            (
                SupportedLanguage::Rust,
                r#"
fn target() -> i32 {
    1
}

fn unrelated() -> &'static str {
    "UNCHANGED"
}
"#,
                "{\n    42\n}",
            ),
            (
                SupportedLanguage::Python,
                r#"
def target():
    return 1

def unrelated():
    return "UNCHANGED"
"#,
                "return 42",
            ),
            (
                SupportedLanguage::JavaScript,
                r#"
function target() {
    return 1;
}

function unrelated() {
    return "UNCHANGED";
}
"#,
                "{\n    return 42;\n}",
            ),
            (
                SupportedLanguage::TypeScript,
                r#"
function target(): number {
    return 1;
}

function unrelated(): string {
    return "UNCHANGED";
}
"#,
                "{\n    return 42;\n}",
            ),
            (
                SupportedLanguage::C,
                r#"
int target(void) {
    return 1;
}

const char *unrelated(void) {
    return "UNCHANGED";
}
"#,
                "{\n    return 42;\n}",
            ),
            (
                SupportedLanguage::Cpp,
                r#"
int target() {
    return 1;
}

const char *unrelated() {
    return "UNCHANGED";
}
"#,
                "{\n    return 42;\n}",
            ),
            (
                SupportedLanguage::Go,
                r#"
package main

func target() int {
    return 1
}

func unrelated() string {
    return "UNCHANGED"
}
"#,
                "{\n    return 42\n}",
            ),
            (
                SupportedLanguage::PHP,
                r#"<?php
function target() {
    return 1;
}

function unrelated() {
    return "UNCHANGED";
}
"#,
                "{\n    return 42;\n}",
            ),
            (
                SupportedLanguage::Zig,
                r#"
fn target() i32 {
    return 1;
}

fn unrelated() []const u8 {
    return "UNCHANGED";
}
"#,
                "{\n    return 42;\n}",
            ),
        ];

        for (language, source, proposed_body) in cases {
            assert!(language.parses_cleanly(source), "{language:?} fixture must parse cleanly");

            let context =
                PolyglotDiffEngine::analyze_source_with_language(language, source, "target")
                    .unwrap_or_else(|e| panic!("{language:?} target discovery failed: {e}"));

            assert!(
                !context.original_body.trim().is_empty(),
                "{language:?} target body must not be empty"
            );

            let result = PolyglotDiffEngine::apply(&context, source, proposed_body)
                .unwrap_or_else(|e| panic!("{language:?} mutation rejected: {e}"));

            assert_ne!(
                result.original_source, result.mutated_source,
                "{language:?} mutation must change the source"
            );

            assert!(result.mutated_source.contains("42"), "{language:?} replacement value missing");

            assert!(
                result.mutated_source.contains("UNCHANGED"),
                "{language:?} unrelated function was altered"
            );

            assert!(
                result.mutated_source.contains("target"),
                "{language:?} target declaration disappeared"
            );
        }
    }

    #[test]
    fn javascript_typescript_harder_constructs_preserve_exact_body_boundary() {
        struct Fixture {
            name: &'static str,
            language: SupportedLanguage,
            source: &'static str,
            replacement: &'static str,
            target_marker: &'static str,
            neighbor_marker: &'static str,
        }

        let fixtures = [
            Fixture {
                name: "javascript_method",
                language: SupportedLanguage::JavaScript,
                source: r#"
class Demo {
    target() {
        return 20;
    }

    after() {
        return 30;
    }
}
"#,
                replacement: "{\n    return 42;\n}",
                target_marker: "target()",
                neighbor_marker: "return 30;",
            },
            Fixture {
                name: "typescript_method",
                language: SupportedLanguage::TypeScript,
                source: r#"
class Demo {
    target(): number {
        return 20;
    }

    after(): number {
        return 30;
    }
}
"#,
                replacement: "{\n    return 42;\n}",
                target_marker: "target(): number",
                neighbor_marker: "return 30;",
            },
            Fixture {
                name: "javascript_arrow",
                language: SupportedLanguage::JavaScript,
                source: r#"
const target = () => {
    return 20;
};

const after = () => {
    return 30;
};
"#,
                replacement: "{\n    return 42;\n}",
                target_marker: "const target",
                neighbor_marker: "return 30;",
            },
            Fixture {
                name: "typescript_arrow",
                language: SupportedLanguage::TypeScript,
                source: r#"
const target = (): number => {
    return 20;
};

const after = (): number => {
    return 30;
};
"#,
                replacement: "{\n    return 42;\n}",
                target_marker: "const target",
                neighbor_marker: "return 30;",
            },
            Fixture {
                name: "javascript_function_expression",
                language: SupportedLanguage::JavaScript,
                source: r#"
const target = function () {
    return 20;
};

const after = function () {
    return 30;
};
"#,
                replacement: "{\n    return 42;\n}",
                target_marker: "const target",
                neighbor_marker: "return 30;",
            },
            Fixture {
                name: "typescript_function_expression",
                language: SupportedLanguage::TypeScript,
                source: r#"
const target = function (): number {
    return 20;
};

const after = function (): number {
    return 30;
};
"#,
                replacement: "{\n    return 42;\n}",
                target_marker: "const target",
                neighbor_marker: "return 30;",
            },
        ];

        for fixture in fixtures {
            assert!(
                fixture.language.parses_cleanly(fixture.source),
                "{} fixture must parse cleanly",
                fixture.name
            );

            let context = PolyglotDiffEngine::analyze_source_with_language(
                fixture.language,
                fixture.source,
                "target",
            )
            .unwrap_or_else(|e| panic!("{} target discovery failed: {e}", fixture.name));

            assert!(
                !context.original_body.trim().is_empty(),
                "{} target body must not be empty",
                fixture.name
            );

            let result = PolyglotDiffEngine::apply(&context, fixture.source, fixture.replacement)
                .unwrap_or_else(|e| panic!("{} mutation rejected: {e}", fixture.name));

            assert!(
                result.mutated_source.contains(fixture.target_marker),
                "{} target declaration disappeared",
                fixture.name
            );

            assert!(
                result.mutated_source.contains("42"),
                "{} replacement value missing",
                fixture.name
            );

            assert!(
                result.mutated_source.contains(fixture.neighbor_marker),
                "{} neighboring function was altered",
                fixture.name
            );

            assert!(
                !result.mutated_source.contains("return 20;"),
                "{} original target body leaked into mutation",
                fixture.name
            );

            assert!(
                fixture.language.parses_cleanly(&result.mutated_source),
                "{} mutated source must parse cleanly",
                fixture.name
            );
        }
    }

    #[test]
    fn duplicate_symbol_is_rejected() {
        let source = r#"
fn target() -> i32 {
    1
}

fn target() -> i32 {
    2
}
"#;

        let result =
            PolyglotDiffEngine::analyze_source(Path::new("duplicate.rs"), source, "target");

        let error = result.expect_err("duplicate target must be rejected");
        assert!(error.to_string().contains("ambiguous"));
    }

    #[test]
    fn stale_diff_context_is_rejected() {
        let source = r#"
fn target() -> i32 {
    2
}
"#;

        let mut context =
            PolyglotDiffEngine::analyze_source(Path::new("stale.rs"), source, "target").unwrap();

        context.original_body = "{\n    999\n}".to_string();

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    42\n}");

        let error = result.expect_err("stale context must be rejected");
        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn inverted_diff_context_range_is_rejected() {
        let source = r#"
fn target() -> i32 {
    2
}
"#;

        let mut context =
            PolyglotDiffEngine::analyze_source(Path::new("range.rs"), source, "target").unwrap();

        context.body_start = context.body_end + 1;

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    42\n}");

        let error = result.expect_err("invalid range must be rejected");
        assert!(error.to_string().contains("invalid UTF-8 byte range"));
    }

    #[test]
    fn malformed_replacement_is_rejected() {
        let source = r#"
fn target() -> i32 {
    2
}
"#;

        let context =
            PolyglotDiffEngine::analyze_source(Path::new("malformed.rs"), source, "target")
                .unwrap();

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    !!!\n}");

        let error = result.expect_err("malformed replacement must be rejected");
        assert!(error.to_string().contains("rejected"));
    }

    #[test]
    fn nearby_function_is_not_modified() {
        let source = r#"
fn before() -> i32 {
    10
}

fn target() -> i32 {
    20
}

fn after() -> i32 {
    30
}
"#;

        let context =
            PolyglotDiffEngine::analyze_source(Path::new("nearby.rs"), source, "target").unwrap();

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    42\n}")
            .expect("mutation must pass");

        assert!(result.mutated_source.contains(
            r#"fn before() -> i32 {
    10
}"#
        ));
        assert!(result.mutated_source.contains(
            r#"fn target() -> i32 {
    42
}"#
        ));
        assert!(result.mutated_source.contains(
            r#"fn after() -> i32 {
    30
}"#
        ));
    }

    #[test]
    fn nested_block_braces_do_not_expand_target_boundary() {
        let source = r#"
fn target() -> i32 {
    if true {
        {
            7
        }
    }

    20
}

fn after() -> i32 {
    30
}
"#;

        let context =
            PolyglotDiffEngine::analyze_source(Path::new("nested.rs"), source, "target").unwrap();

        let original_body = context.original_body.clone();

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    42\n}")
            .expect("mutation must pass");

        assert_eq!(result.mutated_source, source.replacen(&original_body, "{\n    42\n}", 1));
        assert!(result.mutated_source.contains("fn after() -> i32"));
        assert!(result.mutated_source.contains("    30"));
        assert!(!result.mutated_source.contains("7"));
        assert!(result.mutated_source.contains("42"));
    }

    #[test]
    fn unicode_before_target_preserves_exact_body_and_diff() {
        let source = r#"
// Unicode prefix: café 日本語 🚀
fn before() -> i32 {
    10
}

fn target() -> i32 {
    20
}

fn after() -> i32 {
    30
}
"#;

        let context =
            PolyglotDiffEngine::analyze_source(Path::new("unicode.rs"), source, "target").unwrap();

        assert_eq!(context.original_body, "{\n    20\n}");

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    42\n}")
            .expect("mutation must pass");

        assert!(result.mutated_source.contains("café 日本語 🚀"));
        assert!(result.mutated_source.contains("fn before() -> i32"));
        assert!(result.mutated_source.contains("fn after() -> i32"));
        assert!(result.mutated_source.contains("    42"));
        assert!(!result.mutated_source.contains("    20"));

        assert!(result.unified_diff.contains("-    20"));
        assert!(result.unified_diff.contains("+    42"));
    }

    #[test]
    fn unified_diff_is_empty_for_identical_sources() {
        let source = "fn target() -> i32 {\n    42\n}\n";

        assert_eq!(PolyglotDiffEngine::unified_diff(source, source, "target"), "");
    }

    #[test]
    fn unified_diff_is_deterministic_for_replacement() {
        let original = r#"fn target() -> i32 {
    20
}

fn after() -> i32 {
    30
}
"#;

        let mutated = r#"fn target() -> i32 {
    42
}

fn after() -> i32 {
    30
}
"#;

        let expected = r#"--- a/target
+++ b/target
@@ -1,5 +1,5 @@
 fn target() -> i32 {
-    20
+    42
 }
 
 fn after() -> i32 {
"#;

        let first = PolyglotDiffEngine::unified_diff(original, mutated, "target");
        let second = PolyglotDiffEngine::unified_diff(original, mutated, "target");

        assert_eq!(first, expected);
        assert_eq!(second, expected);
    }

    #[test]
    fn adversarial_boundary_matrix_preserves_neighboring_functions() {
        struct Fixture {
            name: &'static str,
            language: SupportedLanguage,
            source: &'static str,
            replacement: &'static str,
            before: &'static str,
            after: &'static str,
        }

        let fixtures = [
            Fixture {
                name: "rust",
                language: SupportedLanguage::Rust,
                source: r#"
fn before() -> i32 {
    10
}

fn target() -> i32 {
    let text = "braces { remain } inside strings";
    // braces { remain } inside comments
    if true {
        20
    } else {
        21
    }
}

fn after() -> i32 {
    30
}
"#,
                replacement: "{\n    42\n}",
                before: "fn before() -> i32 {\n    10\n}",
                after: "fn after() -> i32 {\n    30\n}",
            },
            Fixture {
                name: "python",
                language: SupportedLanguage::Python,
                source: r#"
def before():
    return 10

def target():
    text = "braces { remain } inside strings"
    # braces { remain } inside comments
    if True:
        return 20
    return 21

def after():
    return 30
"#,
                replacement: "def target():\n    return 42",
                before: "def before():\n    return 10",
                after: "def after():\n    return 30",
            },
            Fixture {
                name: "javascript",
                language: SupportedLanguage::JavaScript,
                source: r#"
function before() {
    return 10;
}

function target() {
    const text = "braces { remain } inside strings";
    // braces { remain } inside comments
    if (true) {
        return 20;
    }
    return 21;
}

function after() {
    return 30;
}
"#,
                replacement: "{ return 42; }",
                before: "function before() {\n    return 10;\n}",
                after: "function after() {\n    return 30;\n}",
            },
            Fixture {
                name: "typescript",
                language: SupportedLanguage::TypeScript,
                source: r#"
function before(): number {
    return 10;
}

function target(): number {
    const text = "braces { remain } inside strings";
    // braces { remain } inside comments
    if (true) {
        return 20;
    }
    return 21;
}

function after(): number {
    return 30;
}
"#,
                replacement: "{ return 42; }",
                before: "function before(): number {\n    return 10;\n}",
                after: "function after(): number {\n    return 30;\n}",
            },
            Fixture {
                name: "c",
                language: SupportedLanguage::C,
                source: r#"
int before(void) {
    return 10;
}

int target(void) {
    const char *text = "braces { remain } inside strings";
    /* braces { remain } inside comments */
    if (1) {
        return 20;
    }
    return 21;
}

int after(void) {
    return 30;
}
"#,
                replacement: "{ return 42; }",
                before: "int before(void) {\n    return 10;\n}",
                after: "int after(void) {\n    return 30;\n}",
            },
            Fixture {
                name: "cpp",
                language: SupportedLanguage::Cpp,
                source: r#"
int before() {
    return 10;
}

int target() {
    const char *text = "braces { remain } inside strings";
    // braces { remain } inside comments
    if (true) {
        return 20;
    }
    return 21;
}

int after() {
    return 30;
}
"#,
                replacement: "{ return 42; }",
                before: "int before() {\n    return 10;\n}",
                after: "int after() {\n    return 30;\n}",
            },
            Fixture {
                name: "go",
                language: SupportedLanguage::Go,
                source: r#"
package main

func before() int {
    return 10
}

func target() int {
    text := "braces { remain } inside strings"
    // braces { remain } inside comments
    if true {
        return 20
    }
    return 21
}

func after() int {
    return 30
}
"#,
                replacement: "{ return 42 }",
                before: "func before() int {\n    return 10\n}",
                after: "func after() int {\n    return 30\n}",
            },
            Fixture {
                name: "php",
                language: SupportedLanguage::PHP,
                source: r#"<?php
function before(): int {
    return 10;
}

function target(): int {
    $text = "braces { remain } inside strings";
    // braces { remain } inside comments
    if (true) {
        return 20;
    }
    return 21;
}

function after(): int {
    return 30;
}
"#,
                replacement: "{ return 42; }",
                before: "function before(): int {\n    return 10;\n}",
                after: "function after(): int {\n    return 30;\n}",
            },
            Fixture {
                name: "zig",
                language: SupportedLanguage::Zig,
                source: r#"
fn before() i32 {
    return 10;
}

fn target() i32 {
    const text = "braces { remain } inside strings";
    // braces { remain } inside comments
    if (true) {
        return 20;
    }
    return 21;
}

fn after() i32 {
    return 30;
}
"#,
                replacement: "{ return 42; }",
                before: "fn before() i32 {\n    return 10;\n}",
                after: "fn after() i32 {\n    return 30;\n}",
            },
        ];

        for fixture in fixtures {
            assert!(
                fixture.language.parses_cleanly(fixture.source),
                "{} fixture must parse cleanly",
                fixture.name
            );

            let context = PolyglotDiffEngine::analyze_source_with_language(
                fixture.language,
                fixture.source,
                "target",
            )
            .unwrap_or_else(|error| panic!("{} target discovery failed: {}", fixture.name, error));

            assert!(
                !context.original_body.is_empty(),
                "{} target body must not be empty",
                fixture.name
            );

            let result = PolyglotDiffEngine::apply(&context, fixture.source, fixture.replacement)
                .unwrap_or_else(|error| {
                    panic!("{} target mutation failed: {}", fixture.name, error)
                });

            assert!(
                fixture.language.parses_cleanly(&result.mutated_source),
                "{} mutated source must parse cleanly",
                fixture.name
            );

            assert!(
                result.mutated_source.contains("42"),
                "{} replacement must be present",
                fixture.name
            );

            assert!(
                result.mutated_source.contains(fixture.before),
                "{} neighboring before-function changed unexpectedly",
                fixture.name
            );

            assert!(
                result.mutated_source.contains(fixture.after),
                "{} neighboring after-function changed unexpectedly",
                fixture.name
            );

            assert!(
                !result.mutated_source.contains("return 20"),
                "{} original target body leaked into mutation",
                fixture.name
            );
        }
    }

    #[test]
    fn adversarial_unicode_prefix_matrix_preserves_target_boundary() {
        let fixtures = [
            (
                SupportedLanguage::Rust,
                r#"
// café 日本語 🚀
fn target() -> i32 {
    20
}

fn after() -> i32 {
    30
}
"#,
                "{\n    42\n}",
                "fn after() -> i32",
            ),
            (
                SupportedLanguage::Python,
                r#"
# café 日本語 🚀
def target():
    return 20

def after():
    return 30
"#,
                "def target():\n    return 42",
                "def after():",
            ),
            (
                SupportedLanguage::JavaScript,
                r#"
// café 日本語 🚀
function target() {
    return 20;
}

function after() {
    return 30;
}
"#,
                "{ return 42; }",
                "function after()",
            ),
            (
                SupportedLanguage::TypeScript,
                r#"
// café 日本語 🚀
function target(): number {
    return 20;
}

function after(): number {
    return 30;
}
"#,
                "{ return 42; }",
                "function after()",
            ),
            (
                SupportedLanguage::C,
                r#"
/* café 日本語 🚀 */
int target(void) {
    return 20;
}

int after(void) {
    return 30;
}
"#,
                "{ return 42; }",
                "int after(void)",
            ),
            (
                SupportedLanguage::Cpp,
                r#"
// café 日本語 🚀
int target() {
    return 20;
}

int after() {
    return 30;
}
"#,
                "{ return 42; }",
                "int after()",
            ),
            (
                SupportedLanguage::Go,
                r#"
// café 日本語 🚀
func target() int {
    return 20
}

func after() int {
    return 30
}
"#,
                "{ return 42 }",
                "func after() int",
            ),
            (
                SupportedLanguage::PHP,
                r#"<?php
// café 日本語 🚀
function target(): int {
    return 20;
}

function after(): int {
    return 30;
}
"#,
                "{ return 42; }",
                "function after(): int",
            ),
            (
                SupportedLanguage::Zig,
                r#"
// café 日本語 🚀
fn target() i32 {
    return 20;
}

fn after() i32 {
    return 30;
}
"#,
                "{ return 42; }",
                "fn after() i32",
            ),
        ];

        for (language, source, replacement, neighbor) in fixtures {
            assert!(language.parses_cleanly(source));

            let context =
                PolyglotDiffEngine::analyze_source_with_language(language, source, "target")
                    .unwrap();

            let result = PolyglotDiffEngine::apply(&context, source, replacement).unwrap();

            assert!(result.mutated_source.contains("café"));
            assert!(result.mutated_source.contains("日本語"));
            assert!(result.mutated_source.contains("🚀"));
            assert!(result.mutated_source.contains("42"));
            assert!(result.mutated_source.contains(neighbor));
        }
    }

    #[test]
    fn unicode_before_symbol_preserves_byte_range() {
        let source = r#"
const DESCRIPTION: &str = "日本語 — AthenaFusionX 🚀";

fn target() -> &'static str {
    "old"
}

fn unrelated() -> &'static str {
    "unchanged"
}
"#;

        let path = Path::new("example.rs");
        let context = PolyglotDiffEngine::analyze_source(path, source, "target").unwrap();

        let result = PolyglotDiffEngine::apply(&context, source, "{\n    \"new\"\n}").unwrap();

        assert!(result.mutated_source.contains("\"new\""));
        assert!(result.mutated_source.contains("日本語"));
        assert!(result.mutated_source.contains("\"unchanged\""));
    }
}
