#![allow(unused_imports)]
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Item, ItemFn, ItemStruct, ItemTrait};
use tree_sitter::{Language, Parser, Query, QueryCursor};

#[derive(Serialize)]
struct AstNode {
    name: String,
    node_type: String,
    start_line: usize,
    end_line: usize,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    calls: Vec<String>,
}

#[derive(Serialize)]
struct FileAst {
    file_path: String,
    nodes: Vec<AstNode>,
}

struct CallGraphVisitor {
    pub calls: Vec<String>,
}

impl<'ast> Visit<'ast> for CallGraphVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*node.func {
            if let Some(segment) = expr_path.path.segments.last() {
                self.calls.push(segment.ident.to_string());
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        self.calls.push(node.method.to_string());
        visit::visit_expr_method_call(self, node);
    }
}

fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                if dir_name == "target"
                    || dir_name == ".git"
                    || dir_name == "node_modules"
                    || dir_name == ".nexus_venv"
                    || dir_name == "dist"
                    || dir_name == "build"
                {
                    continue;
                }
                visit_dirs(&path, files)?;
            } else {
                files.push(path);
            }
        }
    }
    Ok(())
}

fn main() {
    println!("==================================================");
    println!(">> NEXUS POLYGLOT MUTATION ENGINE ONLINE");
    println!(">> Slicing AST: Rust (Backend) + Svelte/TS (Frontend)...");
    println!("==================================================\n");

    let mut all_files = Vec::new();
    visit_dirs(Path::new("."), &mut all_files).unwrap();

    let mut mutation_map: Vec<FileAst> = Vec::new();
    let mut rust_nodes = 0;
    let mut python_nodes = 0;
    let mut ui_nodes = 0;

    // Python Parser Setup (Aligned to 0.23 API)
    let mut py_parser = Parser::new();
    let py_language: Language = tree_sitter_python::LANGUAGE.into();
    py_parser.set_language(&py_language).expect("Error loading Python grammar");
    let py_query = Query::new(&py_language, "(function_definition name: (identifier) @name) (class_definition name: (identifier) @name)").unwrap();

    // Frontend (JS/TS) Parser Setup (Aligned to 0.23 API)
    let mut js_parser = Parser::new();
    let js_language: Language = tree_sitter_javascript::LANGUAGE.into();
    js_parser.set_language(&js_language).expect("Error loading JS grammar");
    let js_query = Query::new(&js_language, "(function_declaration name: (identifier) @name) (lexical_declaration (variable_declarator name: (identifier) @name value: (arrow_function)))").unwrap();

    for path in all_files {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        if ext == "rs" {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(syntax_tree) = syn::parse_file(&content) {
                    let mut nodes = Vec::new();
                    for item in syntax_tree.items {
                        match item {
                            Item::Fn(func) => {
                                let span = func.span();
                                let mut visitor = CallGraphVisitor { calls: Vec::new() };
                                visitor.visit_item_fn(&func);
                                let mut unique_calls = visitor.calls;
                                unique_calls.sort();
                                unique_calls.dedup();

                                nodes.push(AstNode {
                                    name: func.sig.ident.to_string(),
                                    node_type: "Rust Function".to_string(),
                                    start_line: span.start().line,
                                    end_line: span.end().line,
                                    calls: unique_calls,
                                });
                            }
                            Item::Struct(st) => {
                                let span = st.span();
                                nodes.push(AstNode {
                                    name: st.ident.to_string(),
                                    node_type: "Rust Struct".to_string(),
                                    start_line: span.start().line,
                                    end_line: span.end().line,
                                    calls: vec![],
                                });
                            }
                            Item::Trait(tr) => {
                                let span = tr.span();
                                nodes.push(AstNode {
                                    name: tr.ident.to_string(),
                                    node_type: "Rust Trait".to_string(),
                                    start_line: span.start().line,
                                    end_line: span.end().line,
                                    calls: vec![],
                                });
                            }
                            _ => {}
                        }
                    }
                    if !nodes.is_empty() {
                        rust_nodes += nodes.len();
                        mutation_map
                            .push(FileAst { file_path: path.to_string_lossy().to_string(), nodes });
                    }
                }
            }
        } else if ext == "py" {
            if let Ok(content) = fs::read_to_string(&path) {
                let tree = py_parser.parse(&content, None).unwrap();
                let mut cursor = QueryCursor::new();
                let matches = cursor.matches(&py_query, tree.root_node(), content.as_bytes());

                let mut nodes = Vec::new();
                for m in matches {
                    for capture in m.captures {
                        let name = capture.node.utf8_text(content.as_bytes()).unwrap().to_string();
                        let node_type =
                            if capture.node.parent().unwrap().kind() == "class_definition" {
                                "Python Class"
                            } else {
                                "Python Function"
                            };
                        let start_line = capture.node.parent().unwrap().start_position().row + 1;
                        let end_line = capture.node.parent().unwrap().end_position().row + 1;

                        nodes.push(AstNode {
                            name,
                            node_type: node_type.to_string(),
                            start_line,
                            end_line,
                            calls: vec![],
                        });
                    }
                }
                if !nodes.is_empty() {
                    python_nodes += nodes.len();
                    mutation_map
                        .push(FileAst { file_path: path.to_string_lossy().to_string(), nodes });
                }
            }
        } else if ext == "js" || ext == "ts" {
            if let Ok(content) = fs::read_to_string(&path) {
                let tree = js_parser.parse(&content, None).unwrap();
                let mut cursor = QueryCursor::new();
                let matches = cursor.matches(&js_query, tree.root_node(), content.as_bytes());

                let mut nodes = Vec::new();
                for m in matches {
                    for capture in m.captures {
                        let name = capture.node.utf8_text(content.as_bytes()).unwrap().to_string();
                        let start_line = capture.node.parent().unwrap().start_position().row + 1;
                        let end_line = capture.node.parent().unwrap().end_position().row + 1;

                        nodes.push(AstNode {
                            name,
                            node_type: "Frontend Logic".to_string(),
                            start_line,
                            end_line,
                            calls: vec![],
                        });
                    }
                }
                if !nodes.is_empty() {
                    ui_nodes += nodes.len();
                    mutation_map
                        .push(FileAst { file_path: path.to_string_lossy().to_string(), nodes });
                }
            }
        } else if ext == "svelte" {
            if let Ok(content) = fs::read_to_string(&path) {
                let lines: Vec<&str> = content.lines().collect();
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let end_line = std::cmp::max(lines.len(), 1);

                mutation_map.push(FileAst {
                    file_path: path.to_string_lossy().to_string(),
                    nodes: vec![AstNode {
                        name,
                        node_type: "Svelte 5 Component".to_string(),
                        start_line: 1,
                        end_line,
                        calls: vec![],
                    }],
                });
                ui_nodes += 1;
            }
        }
    }

    let json_data = serde_json::to_string_pretty(&mutation_map).expect("Failed to serialize");
    let _ = std::fs::create_dir_all("repository_analysis");
    fs::write("repository_analysis/mutation_map.json", json_data).expect("Failed to write map");

    println!(">> POLYGLOT AST ANALYSIS COMPLETE:");
    println!("   - Backend Nodes (Rust/Python): {}", rust_nodes + python_nodes);
    println!("   - Frontend Nodes (Svelte/TS/JS): {}", ui_nodes);
    println!("   - Ready for Full-Stack /mutate God Mode!");
}
