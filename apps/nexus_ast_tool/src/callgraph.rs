//! Cross-file polyglot call-graph taint summary engine.
//!
//! Performs a fixed-point pass across all scanned files to propagate taint
//! from caller sites into callee parameters and records cross-function taint
//! locations for SARIF codeFlows and diagnostics.

use crate::polyglot::SupportedLanguage;
use crate::taint::{TaintAnalyzer, TaintSource};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TaintLocation {
    pub path: PathBuf,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CallerSeed {
    pub param_index: usize,
    pub source: TaintSource,
    pub trace: Vec<String>,
    pub caller_step: TaintLocation,
}

#[derive(Debug, Default)]
pub struct CallGraphIndex {
    defined_functions: HashSet<String>,
    caller_seeds: HashMap<String, Vec<CallerSeed>>,
}

struct FunctionEntry {
    file_idx: usize,
    name: String,
    start_byte: usize,
    end_byte: usize,
}

impl CallGraphIndex {
    pub fn build(files: &[(PathBuf, String, SupportedLanguage)]) -> Self {
        let mut index = Self::default();
        let mut parsed_trees = Vec::with_capacity(files.len());
        let mut functions = Vec::new();

        for (file_idx, (path, source, lang)) in files.iter().enumerate() {
            let mut parser = Parser::new();
            let ts_lang = lang.tree_sitter_language_for_path(path);
            if parser.set_language(&ts_lang).is_err() {
                parsed_trees.push(None);
                continue;
            }
            let Some(tree) = parser.parse(source, None) else {
                parsed_trees.push(None);
                continue;
            };
            collect_functions(tree.root_node(), source, file_idx, &mut functions);
            parsed_trees.push(Some(tree));
        }

        for f in &functions {
            index.defined_functions.insert(f.name.clone());
        }

        // Fixed-point multi-hop propagation (up to 4 hops across caller -> callee chains)
        let mut seen_edges: HashSet<(String, usize, PathBuf, usize)> = HashSet::new();

        for _hop in 0..4 {
            let mut changed = false;

            for f in &functions {
                let Some(Some(tree)) = parsed_trees.get(f.file_idx) else {
                    continue;
                };
                let (ref path, ref source, lang) = files[f.file_idx];
                let Some(func_node) = tree
                    .root_node()
                    .descendant_for_byte_range(f.start_byte, f.end_byte)
                else {
                    continue;
                };

                let (has_callers, raw_seeds) = index.seeds_for(&f.name);
                let simple_seeds: Vec<(usize, TaintSource, Vec<String>)> = raw_seeds
                    .iter()
                    .map(|s| (s.param_index, s.source, s.trace.clone()))
                    .collect();

                let mut analyzer = TaintAnalyzer::new(source, lang);
                analyzer.analyze_function_with_callers(
                    func_node,
                    &simple_seeds,
                    has_callers,
                    usize::MAX,
                );

                let mut calls = Vec::new();
                collect_call_nodes(func_node, &mut calls);

                for call_node in calls {
                    let Some(callee_name) = extract_callee_name(call_node, source) else {
                        continue;
                    };
                    if callee_name == f.name || !index.defined_functions.contains(&callee_name) {
                        continue;
                    }

                    let args = extract_call_arguments(call_node);
                    let call_line = call_node.start_position().row + 1;

                    // Record edge presence even when arguments are clean so we know the function has callers.
                    index.caller_seeds.entry(callee_name.clone()).or_default();

                    for (arg_idx, arg_node) in args.into_iter().enumerate() {
                        let report = analyzer.check_expression(arg_node);
                        if !report.tainted {
                            continue;
                        }
                        let edge_key =
                            (callee_name.clone(), arg_idx, path.clone(), call_line);
                        if !seen_edges.insert(edge_key) {
                            continue;
                        }

                        let mut trace = report.path.clone();
                        trace.insert(
                            0,
                            format!(
                                "{} -> {}() at {}:{}",
                                f.name,
                                callee_name,
                                path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
                                call_line
                            ),
                        );

                        let caller_step = TaintLocation {
                            path: path.clone(),
                            line: call_line,
                            message: format!(
                                "Tainted argument #{} passed from `{}()` to `{}()`",
                                arg_idx + 1,
                                f.name,
                                callee_name
                            ),
                        };

                        index
                            .caller_seeds
                            .entry(callee_name.clone())
                            .or_default()
                            .push(CallerSeed {
                                param_index: arg_idx,
                                source: report.source.unwrap_or(TaintSource::Unknown),
                                trace,
                                caller_step,
                            });
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        index
    }

    /// Returns `(has_known_callers, tainted_caller_seeds)`.
    pub fn seeds_for(&self, func_name: &str) -> (bool, Vec<CallerSeed>) {
        match self.caller_seeds.get(func_name) {
            Some(seeds) => (true, seeds.clone()),
            None => (false, Vec::new()),
        }
    }
}

pub fn enclosing_function_name(func_node: Node, source: &str) -> Option<String> {
    extract_function_name(func_node, source)
}

fn collect_functions(
    node: Node,
    source: &str,
    file_idx: usize,
    out: &mut Vec<FunctionEntry>,
) {
    if is_function_node(node.kind()) {
        if let Some(name) = extract_function_name(node, source) {
            out.push(FunctionEntry {
                file_idx,
                name,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, source, file_idx, out);
    }
}

fn is_function_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition"
            | "function_declaration"
            | "method_declaration"
            | "method_definition"
            | "function_item"
    )
}

fn extract_function_name(node: Node, source: &str) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = source.get(name_node.byte_range())?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    // C / C++ function_definition -> declarator: function_declarator -> declarator: identifier
    if let Some(decl) = node.child_by_field_name("declarator") {
        if let Some(inner) = decl.child_by_field_name("declarator") {
            let name = source.get(inner.byte_range())?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn collect_call_nodes<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    if matches!(
        node.kind(),
        "call" | "call_expression" | "function_call_expression" | "member_call_expression"
    ) {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_call_nodes(child, out);
    }
}

fn extract_callee_name(call_node: Node, source: &str) -> Option<String> {
    let func_part = call_node
        .child_by_field_name("function")
        .or_else(|| call_node.child_by_field_name("name"))?;
    let raw = source.get(func_part.byte_range())?.trim();
    let last = raw
        .rsplit(|c: char| c == '.' || c == ':' || c == '>')
        .next()?
        .trim();
    if last.is_empty() || !last.chars().all(|c| c.is_alphanumeric() || c == '_') {
        None
    } else {
        Some(last.to_string())
    }
}

fn extract_call_arguments<'a>(call_node: Node<'a>) -> Vec<Node<'a>> {
    let Some(args_node) = call_node.child_by_field_name("arguments") else {
        return Vec::new();
    };
    let mut cursor = args_node.walk();
    args_node
        .named_children(&mut cursor)
        .filter(|n| n.kind() != "comment")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_file_taint_propagates_and_filters_safe_callers() {
        let files = vec![
            (
                PathBuf::from("routes.py"),
                r#"
def signup_route():
    code = request.form.get("code")
    return validate_token(code)

def health_check():
    return safe_helper("SELECT 1")
"#
                .to_string(),
                SupportedLanguage::Python,
            ),
            (
                PathBuf::from("dao.py"),
                r#"
def validate_token(token):
    return session.execute(text(f"SELECT * FROM users WHERE code = '{token}'"))

def safe_helper(sql):
    return session.execute(text(f"{sql}"))
"#
                .to_string(),
                SupportedLanguage::Python,
            ),
        ];

        let cg = CallGraphIndex::build(&files);

        let (has_val_callers, val_seeds) = cg.seeds_for("validate_token");
        assert!(has_val_callers);
        assert!(
            val_seeds.iter().any(|s| s.param_index == 0),
            "validate_token(token) should receive tainted arg 0 from signup_route"
        );

        let (has_safe_callers, safe_seeds) = cg.seeds_for("safe_helper");
        assert!(has_safe_callers);
        assert!(
            safe_seeds.is_empty(),
            "safe_helper called only with constant string must have 0 tainted seeds"
        );
    }

    #[test]
    fn multi_hop_propagation_across_three_functions() {
        let files = vec![(
            PathBuf::from("chain.py"),
            r#"
def endpoint():
    raw = request.args.get("q")
    return service(raw)

def service(val):
    return dao(val)

def dao(q):
    return cursor.execute(f"SELECT * FROM t WHERE a = '{q}'")
"#
            .to_string(),
            SupportedLanguage::Python,
        )];

        let cg = CallGraphIndex::build(&files);
        let (has_dao_callers, dao_seeds) = cg.seeds_for("dao");
        assert!(has_dao_callers);
        assert!(
            dao_seeds.iter().any(|s| s.param_index == 0),
            "taint must propagate endpoint -> service -> dao"
        );
    }
}
