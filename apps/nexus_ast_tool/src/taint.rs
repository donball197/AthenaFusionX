//! Advanced intra-procedural taint analysis for all supported languages.
//!
//! Tracks user-controlled input through variable assignments, string
//! concatenation, template literals, and function calls.
//! Recognizes language-specific sanitizers and records taint paths for evidence.

use crate::polyglot::SupportedLanguage;
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

/// Where user-controlled input originates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaintSource {
    Body,
    Query,
    Params,
    Headers,
    Cookies,
    Url,
    PhpInput,
    Request,
    Unknown,
}

impl std::fmt::Display for TaintSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Body => write!(f, "request body"),
            Self::Query => write!(f, "query parameter"),
            Self::Params => write!(f, "URL parameter"),
            Self::Headers => write!(f, "request header"),
            Self::Cookies => write!(f, "cookie"),
            Self::Url => write!(f, "URL"),
            Self::PhpInput => write!(f, "php://input"),
            Self::Request => write!(f, "request object"),
            Self::Unknown => write!(f, "user input"),
        }
    }
}

/// A tracked taint fact for a variable.
#[derive(Debug, Clone)]
pub struct TaintFact {
    pub name: String,
    pub source: TaintSource,
    pub path: Vec<String>,
}

/// Result of checking an expression for taint.
#[derive(Debug, Clone)]
pub struct TaintReport {
    pub tainted: bool,
    pub source: Option<TaintSource>,
    pub path: Vec<String>,
    pub sanitized: bool,
}

/// Intra-procedural taint analyzer with sanitizer awareness.
pub struct TaintAnalyzer<'a> {
    source: &'a str,
    language: SupportedLanguage,
    tainted: HashMap<String, TaintFact>,
    sanitized: HashSet<String>,
}

impl<'a> TaintAnalyzer<'a> {
    pub fn new(source: &'a str, language: SupportedLanguage) -> Self {
        Self {
            source,
            language,
            tainted: HashMap::new(),
            sanitized: HashSet::new(),
        }
    }

    /// Walk a function body and track all variable assignments.
    pub fn analyze_function(&mut self, func_node: Node) {
        self.analyze_function_until(func_node, usize::MAX);
    }

    /// Statement-ordered analysis up to `max_row` (0-based) so later statements
    /// cannot retroactively overwrite taint at an earlier sink.
    pub fn analyze_function_until(&mut self, func_node: Node, max_row: usize) {
        self.seed_parameters(func_node);
        self.walk_until(func_node, max_row);
    }

    /// Analyze a function using cross-file caller taint summaries when available.
    pub fn analyze_function_with_callers(
        &mut self,
        func_node: Node,
        caller_seeds: &[(usize, TaintSource, Vec<String>)],
        has_known_callers: bool,
        max_row: usize,
    ) {
        if has_known_callers {
            let param_names = self.extract_parameter_names(func_node);
            for (idx, source, trace) in caller_seeds {
                if let Some(name) = param_names.get(*idx) {
                    if !matches!(
                        name.as_str(),
                        "self" | "cls" | "this" | "session" | "db" | "conn" | "cursor"
                    ) {
                        self.tainted.insert(
                            name.clone(),
                            TaintFact {
                                name: name.clone(),
                                source: *source,
                                path: trace.clone(),
                            },
                        );
                    }
                }
            }
        } else {
            self.seed_parameters(func_node);
        }
        self.walk_until(func_node, max_row);
    }

    /// Check if an AST node references any tainted data.
    pub fn check_expression(&self, node: Node) -> TaintReport {
        let mut report = TaintReport {
            tainted: false,
            source: None,
            path: vec![],
            sanitized: false,
        };
        self.walk_for_taint(node, &mut report);
        report
    }

    /// Check if raw text references any tainted data or source.
    pub fn check_text(&self, text: &str) -> TaintReport {
        if let Some(source) = self.detect_source(text) {
            return TaintReport {
                tainted: true,
                source: Some(source),
                path: vec![text.trim().to_string()],
                sanitized: false,
            };
        }
        for (name, fact) in &self.tainted {
            if !self.sanitized.contains(name) && Self::contains_ident(text, name.as_str()) {
                return TaintReport {
                    tainted: true,
                    source: Some(fact.source),
                    path: fact.path.clone(),
                    sanitized: false,
                };
            }
        }
        TaintReport { tainted: false, source: None, path: vec![], sanitized: false }
    }

    // ======== Internal ========

    fn walk_until(&mut self, node: Node, max_row: usize) {
        if node.start_position().row > max_row {
            return;
        }
        match node.kind() {
            "variable_declarator"
            | "assignment_expression"
            | "assignment"
            | "short_var_declaration"
            | "assignment_statement"
            | "var_spec"
            | "let_declaration"
            | "init_declarator" => {
                self.handle_assignment(node);
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_until(child, max_row);
        }
    }

    fn handle_assignment(&mut self, node: Node) {
        let left = node
            .child_by_field_name("left")
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.child_by_field_name("pattern"))
            .or_else(|| node.child_by_field_name("declarator"));
        let right = node
            .child_by_field_name("right")
            .or_else(|| node.child_by_field_name("value"));
        let (Some(left), Some(right)) = (left, right) else { return };

        if matches!(left.kind(), "object_pattern" | "array_pattern") {
            let mut names = Vec::new();
            self.collect_lhs_idents(left, &mut names);
            for name in names {
                self.assign_name(node, name, right);
            }
            return;
        }

        if left.kind() == "expression_list" {
            let mut lcur = left.walk();
            let lhs_nodes: Vec<Node> = left.named_children(&mut lcur).collect();
            let mut rcur = right.walk();
            let rhs_nodes: Vec<Node> = if right.kind() == "expression_list" {
                right.named_children(&mut rcur).collect()
            } else {
                vec![right]
            };

            for (idx, lhs) in lhs_nodes.iter().enumerate() {
                let rhs = if rhs_nodes.len() == lhs_nodes.len() {
                    rhs_nodes[idx]
                } else {
                    right
                };
                self.assign_pair(node, *lhs, rhs);
            }
            return;
        }

        self.assign_pair(node, left, right);
    }

    fn collect_lhs_idents(&self, node: Node, out: &mut Vec<String>) {
        match node.kind() {
            "identifier" | "shorthand_property_identifier_pattern" => {
                let name = self.node_text(node).trim().trim_start_matches('$').to_string();
                if !name.is_empty() && name != "_" {
                    out.push(name);
                }
                return;
            }
            "pair_pattern" => {
                if let Some(val) = node.child_by_field_name("value") {
                    self.collect_lhs_idents(val, out);
                }
                return;
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_lhs_idents(child, out);
        }
    }

    fn assign_pair(&mut self, stmt_node: Node, left: Node, right: Node) {
        let left_text = self
            .node_text(left)
            .trim()
            .trim_start_matches(['$', '*', '&'])
            .trim()
            .to_string();
        if left_text.is_empty() || left_text == "_" {
            return;
        }
        self.assign_name(stmt_node, left_text, right);
    }

    fn assign_name(&mut self, stmt_node: Node, left_text: String, right: Node) {
        let right_text = self.node_text(right);

        // 1. Sanitizer call → mark clean
        if self.is_sanitizer_call(right) {
            self.sanitized.insert(left_text.clone());
            self.tainted.remove(&left_text);
            return;
        }

        // 2. Direct taint source on right side
        if let Some(source) = self.detect_source(&right_text) {
            self.tainted.insert(left_text.clone(), TaintFact {
                name: left_text.clone(),
                source,
                path: vec![left_text.clone(), right_text.trim().to_string()],
            });
            self.sanitized.remove(&left_text);
            return;
        }

        // 3. Propagate from tainted variables in right subtree
        if let Some(fact) = self.find_tainted_in_subtree(right) {
            let mut path = fact.path.clone();
            path.insert(0, left_text.clone());
            self.tainted.insert(left_text.clone(), TaintFact {
                name: left_text.clone(),
                source: fact.source,
                path,
            });
            self.sanitized.remove(&left_text);
            return;
        }

        // 4. Clean assignment (do not kill taint inside conditional branches)
        let in_branch = {
            let mut cur = stmt_node.parent();
            let mut found = false;
            while let Some(parent) = cur {
                if matches!(
                    parent.kind(),
                    "if_statement" | "elif_clause" | "else_clause" | "conditional_expression"
                ) {
                    found = true;
                    break;
                }
                cur = parent.parent();
            }
            found
        };
        if !in_branch {
            self.tainted.remove(&left_text);
        }
    }

    fn walk_for_taint(&self, node: Node, report: &mut TaintReport) {
        if report.tainted {
            return;
        }

        let text = self.node_text(node);

        // Direct source detection on any node
        if let Some(source) = self.detect_source(text) {
            report.tainted = true;
            report.source = Some(source);
            report.path = vec![text.trim().to_string()];
            return;
        }

        // Variable reference check
        match node.kind() {
            "identifier" | "variable_name" | "shorthand_property_identifier_pattern" => {
                let name = text.trim().trim_start_matches('$');
                if self.sanitized.contains(name) {
                    report.sanitized = true;
                    return;
                }
                if let Some(fact) = self.tainted.get(name) {
                    report.tainted = true;
                    report.source = Some(fact.source);
                    report.path = fact.path.clone();
                    return;
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_for_taint(child, report);
            if report.tainted {
                return;
            }
        }
    }

    fn find_tainted_in_subtree(&self, node: Node) -> Option<TaintFact> {
        let text = self.node_text(node);

        if let Some(source) = self.detect_source(text) {
            return Some(TaintFact {
                name: text.trim().to_string(),
                source,
                path: vec![text.trim().to_string()],
            });
        }

        for (name, fact) in &self.tainted {
            if !self.sanitized.contains(name) && Self::contains_ident(text, name.as_str()) {
                return Some(fact.clone());
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(fact) = self.find_tainted_in_subtree(child) {
                return Some(fact);
            }
        }
        None
    }

    fn is_sanitizer_call(&self, node: Node) -> bool {
        let text = self.node_text(node);
        // Check the node itself and its immediate call children
        if let Some(name) = self.extract_call_name(text) {
            return is_sanitizer(&name, self.language);
        }
        // Check children for call expressions
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let k = child.kind();
            if k.contains("call") {
                let child_text = self.node_text(child);
                if let Some(name) = self.extract_call_name(child_text) {
                    return is_sanitizer(&name, self.language);
                }
            }
        }
        false
    }

    fn extract_call_name(&self, text: &str) -> Option<String> {
        let trimmed = text.trim();
        let paren_pos = trimmed.find('(')?;
        let call_part = trimmed[..paren_pos].trim();
        // Handle member calls: obj.method, ns.func.method
        let name = call_part.split('.').last()?.trim().trim_start_matches('$');
        if name.is_empty() { None } else { Some(name.to_string()) }
    }

    /// Language-aware detection of user input sources.
    fn detect_source(&self, text: &str) -> Option<TaintSource> {
        let t = text.trim();

        // Body / POST data
        if t.contains("req.body") || t.contains("request.body") || t.contains("ctx.request.body")
            || t.contains("$_POST") || t.contains("$_REQUEST") || t.contains("req.form")
            || t.contains("request.form") || t.contains("request.values")
            || t.contains("request.get_json") || t.contains("request.json")
            || t.contains("request.data") || t.contains("request.files")
            || t.contains("r.FormValue") || t.contains("r.PostFormValue")
            || t.contains("r.Form") || t.contains("r.PostForm") || t.contains("r.Body")
            || t.contains("c.PostForm") || t.contains("c.Bind") || t.contains("c.ShouldBind") {
            return Some(TaintSource::Body);
        }
        // Query parameters
        if t.contains("req.query") || t.contains("request.query") || t.contains("$_GET")
            || t.contains("ctx.query") || t.contains("req.searchParams")
            || t.contains("request.args") || t.contains("request.query_params")
            || t.contains("r.URL.Query") || t.contains("req.URL.Query")
            || t.contains("c.Query") || t.contains("c.DefaultQuery") {
            return Some(TaintSource::Query);
        }
        // URL parameters
        if t.contains("req.params") || t.contains("request.params") || t.contains("ctx.params")
            || t.contains("request.path_params")
            || t.contains("r.PathValue") || t.contains("mux.Vars")
            || t.contains("chi.URLParam") || t.contains("c.Param") {
            return Some(TaintSource::Params);
        }
        // Headers
        if t.contains("req.headers") || t.contains("request.headers") || t.contains("$_SERVER")
            || t.contains("ctx.headers")
            || t.contains("r.Header") || t.contains("c.GetHeader") {
            return Some(TaintSource::Headers);
        }
        // Cookies
        if t.contains("req.cookies") || t.contains("request.cookies") || t.contains("$_COOKIE")
            || t.contains("r.Cookie") || t.contains("c.Cookie") {
            return Some(TaintSource::Cookies);
        }
        // URL / location
        if t.contains("req.url") || t.contains("request.url") || t.contains("location.href")
            || t.contains("location.search") {
            return Some(TaintSource::Url);
        }
        // PHP raw input
        if t.contains("php://input") {
            return Some(TaintSource::PhpInput);
        }
        // C / C++ external inputs
        if t.contains("getenv(") || t.contains("fgets(") || t.contains("gets(")
            || t.contains("scanf(") || t.contains("recv(") || t.contains("recvfrom(")
            || t.contains("argv[") {
            return Some(TaintSource::Unknown);
        }
        // Generic request objects
        if t.starts_with("req.") || t.starts_with("request.") || t.starts_with("ctx.")
            || t.starts_with("$_") {
            return Some(TaintSource::Request);
        }
        None
    }

    pub fn extract_parameter_names(&self, func_node: Node) -> Vec<String> {
        let mut out = Vec::new();
        let Some(params) = func_node.child_by_field_name("parameters") else {
            return out;
        };
        self.collect_param_names(params, &mut out);
        out
    }

    fn collect_param_names(&self, node: Node, out: &mut Vec<String>) {
        let kind = node.kind();
        if matches!(kind, "type" | "type_annotation" | "return_type") {
            return;
        }
        match kind {
            "identifier" | "variable_name" | "shorthand_property_identifier_pattern" => {
                let name = self.node_text(node).trim().trim_start_matches('$').to_string();
                if !name.is_empty() {
                    out.push(name);
                }
                return;
            }
            "typed_parameter" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        self.collect_param_names(child, out);
                        break;
                    }
                }
                return;
            }
            "default_parameter" | "typed_default_parameter" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    self.collect_param_names(name_node, out);
                }
                return;
            }
            "required_parameter" | "optional_parameter" | "parameter" => {
                if let Some(pattern) = node.child_by_field_name("pattern") {
                    self.collect_param_names(pattern, out);
                }
                return;
            }
            "assignment_pattern" => {
                if let Some(left) = node.child_by_field_name("left") {
                    self.collect_param_names(left, out);
                }
                return;
            }
            "parameter_declaration" | "variadic_parameter_declaration" => {
                let mut cursor = node.walk();
                let mut found = false;
                for child in node.children_by_field_name("name", &mut cursor) {
                    found = true;
                    self.collect_param_names(child, out);
                }
                if !found {
                    if let Some(decl) = node.child_by_field_name("declarator") {
                        self.collect_param_names(decl, out);
                    }
                }
                return;
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_param_names(child, out);
        }
    }

    fn seed_parameters(&mut self, func_node: Node) {
        let Some(params) = func_node.child_by_field_name("parameters") else {
            return;
        };
        self.collect_param_idents(params);
    }

    fn collect_param_idents(&mut self, node: Node) {
        let kind = node.kind();
        if matches!(kind, "type" | "type_annotation" | "return_type") {
            return;
        }
        match kind {
            "identifier" | "variable_name" | "shorthand_property_identifier_pattern" => {
                let name = self.node_text(node).trim().trim_start_matches('$').to_string();
                if !name.is_empty()
                    && !matches!(
                        name.as_str(),
                        "self" | "cls" | "this" | "session" | "db" | "conn" | "cursor"
                    )
                {
                    self.tainted.insert(
                        name.clone(),
                        TaintFact {
                            name: name.clone(),
                            source: TaintSource::Unknown,
                            path: vec![name, "<parameter>".to_string()],
                        },
                    );
                }
                return;
            }
            "typed_parameter" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        self.collect_param_idents(child);
                        break;
                    }
                }
                return;
            }
            "default_parameter" | "typed_default_parameter" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    self.collect_param_idents(name_node);
                }
                return;
            }
            "required_parameter" | "optional_parameter" | "parameter" => {
                if let Some(pattern) = node.child_by_field_name("pattern") {
                    self.collect_param_idents(pattern);
                }
                return;
            }
            "assignment_pattern" => {
                if let Some(left) = node.child_by_field_name("left") {
                    self.collect_param_idents(left);
                }
                return;
            }
            "parameter_declaration" | "variadic_parameter_declaration" => {
                let mut cursor = node.walk();
                let mut found = false;
                for child in node.children_by_field_name("name", &mut cursor) {
                    found = true;
                    self.collect_param_idents(child);
                }
                if !found {
                    if let Some(decl) = node.child_by_field_name("declarator") {
                        self.collect_param_idents(decl);
                    }
                }
                return;
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_param_idents(child);
        }
    }

    fn contains_ident(haystack: &str, ident: &str) -> bool {
        if ident.is_empty() {
            return false;
        }
        let bytes = haystack.as_bytes();
        let is_ident_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(ident) {
            let abs = start + pos;
            let before_ok = abs == 0 || !is_ident_byte(bytes[abs - 1]);
            let after = abs + ident.len();
            let after_ok = after == bytes.len() || !is_ident_byte(bytes[after]);
            if before_ok && after_ok {
                return true;
            }
            start = abs + ident.len().max(1);
        }
        false
    }

    fn node_text(&self, node: Node) -> &str {
        &self.source[node.byte_range()]
    }
}

/// Find the nearest enclosing function-like node.
pub fn find_enclosing_function(node: Node) -> Option<Node> {
    let mut n = node;
    while let Some(p) = n.parent() {
        if matches!(
            p.kind(),
            "function_declaration" | "function_expression" | "arrow_function"
                | "method_definition" | "function_item" | "function_definition"
                | "method_declaration" | "func_literal"
        ) {
            return Some(p);
        }
        n = p;
    }
    None
}

/// Known sanitizer functions per language.
/// If user input passes through these, the result is considered clean.
pub fn is_sanitizer(func_name: &str, language: SupportedLanguage) -> bool {
    match language {
        SupportedLanguage::PHP => matches!(
            func_name,
            "htmlspecialchars" | "htmlentities" | "mysqli_real_escape_string"
                | "urlencode" | "filter_var" | "strip_tags" | "intval"
                | "floatval" | "preg_quote" | "escapeshellarg"
                | "escapeshellcmd" | "addslashes" | "pg_escape_string"
                | "filter_input" | "json_encode" | "real_escape_string"
        ),
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript => matches!(
            func_name,
            "encodeURIComponent" | "encodeURI" | "escape"
                | "sanitize" | "DOMPurify" | "xss" | "htmlspecialchars"
                | "Number" | "parseInt" | "parseFloat"
                | "escapeHtml" | "encodeHTML" | "sanitizeHtml" | "escapeSQL"
        ),
        SupportedLanguage::Python => matches!(
            func_name,
            "escape" | "html_escape" | "shlex" | "quote"
                | "int" | "float" | "bleach" | "clean"
                | "markupsafe" | "cgi_escape" | "quote_plus" | "parameterize"
        ),
        SupportedLanguage::Rust => matches!(
            func_name,
            "escape" | "html_escape" | "sanitize" | "parse" | "from_str"
        ),
        SupportedLanguage::Go => matches!(
            func_name,
            "EscapeString" | "QueryEscape" | "PathEscape"
                | "HTMLEscape" | "ParseInt" | "Atoi"
        ),
        SupportedLanguage::C | SupportedLanguage::Cpp => matches!(
            func_name,
            "snprintf" | "strncpy" | "strlcpy" | "sanitize"
                | "atoi" | "atol" | "strtol" | "strtoul"
        ),
        SupportedLanguage::Zig => matches!(
            func_name,
            "parseInt" | "parseFloat" | "escape" | "sanitize"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_js(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).unwrap();
        parser.parse(source, None).unwrap()
    }

    fn find_func(tree: &tree_sitter::Tree) -> tree_sitter::Node<'_> {
        find_enclosing_function(tree.root_node().child(0).unwrap_or(tree.root_node()))
            .unwrap_or(tree.root_node())
    }

    #[test]
    fn detects_req_body() {
        let src = "function f() { let x = req.body; }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        assert!(a.tainted.contains_key("x"), "x should be tainted from req.body");
    }

    #[test]
    fn detects_req_query() {
        let src = "function f() { let id = req.query.id; }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        assert!(a.tainted.contains_key("id"), "id should be tainted from req.query");
    }

    #[test]
    fn propagation_chain() {
        let src = "function f() { let a = req.body; let b = a; let c = b; }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        assert!(a.tainted.contains_key("a"));
        assert!(a.tainted.contains_key("b"), "taint propagates a -> b");
        assert!(a.tainted.contains_key("c"), "taint propagates b -> c");
    }

    #[test]
    fn concatenation_taints() {
        let src = "function f() { let sql = \"SELECT * FROM u WHERE id = \" + req.query.id; }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        assert!(a.tainted.contains_key("sql"), "concat with req.query should taint sql");
    }

    #[test]
    fn template_literal_taints() {
        let src = "function f() { let sql = `SELECT * FROM u WHERE id = ${req.query.id}`; }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        assert!(a.tainted.contains_key("sql"), "template literal with req.query should taint sql");
    }

    #[test]
    fn sanitizer_clears_taint() {
        let src = "function f() { let safe = parseInt(req.query.id, 10); }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        assert!(a.sanitized.contains("safe"), "parseInt should sanitize");
        assert!(!a.tainted.contains_key("safe"), "sanitized var should not be tainted");
    }

    #[test]
    fn clean_literal_not_tainted() {
        let src = "function f() { let id = 42; }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        assert!(!a.tainted.contains_key("id"));
    }

    #[test]
    fn check_expression_finds_tainted_var() {
        let src = "function f() { let input = req.body; db.query(input); }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        let root = tree.root_node();
        let report = a.check_expression(root);
        assert!(report.tainted, "expression tree contains tainted variable input");
    }

    #[test]
    fn path_is_recorded() {
        let src = "function f() { let a = req.query.x; }";
        let tree = parse_js(src);
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::JavaScript);
        a.analyze_function(find_func(&tree));
        let fact = a.tainted.get("a").unwrap();
        assert!(!fact.path.is_empty(), "taint path should be recorded");
    }

    #[test]
    fn go_url_query_and_sprintf_stay_tainted() {
        let src = r#"
package main
func h(w http.ResponseWriter, r *http.Request) {
    email := r.URL.Query().Get("email")
    query := fmt.Sprintf("SELECT * FROM users WHERE email = '%s'", email)
}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into()).unwrap();
        let tree = parser.parse(src, None).unwrap();
        let mut a = TaintAnalyzer::new(src, SupportedLanguage::Go);
        a.analyze_function(find_func(&tree));
        assert!(a.tainted.contains_key("email"), "email from r.URL.Query().Get must be tainted");
        assert!(!a.sanitized.contains("email"), "r.URL.Query() must not be treated as a sanitizer");
        assert!(a.tainted.contains_key("query"), "fmt.Sprintf with tainted email must taint query");
    }
}
