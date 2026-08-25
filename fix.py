#!/usr/bin/env python3
import sys

edits = []

# --- File 1: polyglot.rs — add verify_statement_threading ---
poly_path = "apps/nexus_ast_tool/src/polyglot.rs"
with open(poly_path, "r") as f:
    poly = f.read()

poly_old = '''    best.map(|(s, e, _, t)| (s + 1, e + 1, t)) // back to 1-based
}

pub struct PolyglotScanner;'''

poly_new = '''    best.map(|(s, e, _, t)| (s + 1, e + 1, t)) // back to 1-based
}

/// Verifies that a `mysqli_prepare()` result is the SAME statement object
/// threaded into both `mysqli_stmt_bind_param()` and `mysqli_stmt_execute()`,
/// not just that those two function names appear somewhere in the source.
/// Confirmed real failure mode: a model emitted `mysqli_stmt_prepare($con, $q)`
/// (wrong function -- takes a stmt object, not a connection, as its first
/// arg) and called bind_param/execute on the connection handle instead of a
/// captured statement variable. That patch is syntactically valid PHP and
/// contains both substrings, but fatally errors at runtime (TypeError:
/// mysqli_stmt_bind_param() expects parameter 1 to be mysqli_stmt, mysqli
/// given) -- the query never actually executes. PHP-only; returns true (no
/// opinion) for any other language.
pub fn verify_statement_threading(source: &str, lang: SupportedLanguage) -> bool {
    if lang != SupportedLanguage::PHP {
        return true;
    }
    let ts_lang: Language = lang.tree_sitter_language();
    let mut parser = Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return false; // fail closed
    }
    let Some(tree) = parser.parse(source, None) else { return false };

    let run_query = |query_src: &str, capture_name: &str| -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        let Ok(query) = Query::new(&ts_lang, query_src) else { return out };
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        for m in matches {
            for capture in m.captures {
                if query.capture_names()[capture.index as usize] == capture_name {
                    if let Ok(text) = capture.node.utf8_text(source.as_bytes()) {
                        out.insert(text.to_string());
                    }
                }
            }
        }
        out
    };

    let prepared_vars = run_query(
        "(assignment_expression \\
            left: (variable_name) @stmt_var \\
            right: (function_call_expression \\
                function: (name) @fname \\
                (#match? @fname \\"^mysqli_prepare$\\") \\
            ) \\
         )",
        "stmt_var",
    );

    let bind_vars = run_query(
        "(function_call_expression \\
            function: (name) @fname \\
            (#match? @fname \\"^mysqli_stmt_bind_param$\\") \\
            arguments: (arguments . (argument (variable_name) @bind_var)) \\
         )",
        "bind_var",
    );

    let exec_vars = run_query(
        "(function_call_expression \\
            function: (name) @fname \\
            (#match? @fname \\"^mysqli_stmt_execute$\\") \\
            arguments: (arguments . (argument (variable_name) @exec_var)) \\
         )",
        "exec_var",
    );

    prepared_vars.iter().any(|v| bind_vars.contains(v) && exec_vars.contains(v))
}

pub struct PolyglotScanner;'''

edits.append((poly_path, poly_old, poly_new))

# --- File 2: main.rs — call the new check alongside the existing substring
#     check, at both the multi-site and single-site param_ok sites ---
main_path = "apps/nexus_ast_tool/src/main.rs"
with open(main_path, "r") as f:
    main_src = f.read()

# Multi-site (deeper indentation, appears once inside the taint branch)
main_old_multi = '''                                        let syntax_ok = ts_ok && lint.unwrap_or(true);

                                        let param_ok = has_real_parameterization(&new_source);

                                        let ast_ok = if syntax_ok {'''

main_new_multi = '''                                        let syntax_ok = ts_ok && lint.unwrap_or(true);

                                        let param_ok = has_real_parameterization(&new_source)
                                            && nexus_ast_tool::polyglot::verify_statement_threading(&new_source, finding.language);

                                        let ast_ok = if syntax_ok {'''

edits.append((main_path, main_old_multi, main_new_multi))

# Single-site (shallower indentation, appears once near the end of the
# non-taint patch path)
main_old_single = '''                                    let param_ok = has_real_parameterization(&new_source);

                                    if syntax_ok && param_ok && ast_ok {'''

main_new_single = '''                                    let param_ok = has_real_parameterization(&new_source)
                                        && nexus_ast_tool::polyglot::verify_statement_threading(&new_source, finding.language);

                                    if syntax_ok && param_ok && ast_ok {'''

edits.append((main_path, main_old_single, main_new_single))

# Apply, per-file, checking each edit independently
from collections import defaultdict
contents = {}
for path, old, new in edits:
    if path not in contents:
        with open(path, "r") as f:
            contents[path] = f.read()
    count = contents[path].count(old)
    if count == 0:
        print(f"FAIL: anchor not found in {path}")
        print("---- looking for ----")
        print(old)
        sys.exit(1)
    if count > 1:
        print(f"FAIL: anchor not unique in {path} ({count} matches)")
        sys.exit(1)
    contents[path] = contents[path].replace(old, new)

for path, content in contents.items():
    with open(path, "w") as f:
        f.write(content)
    print(f"OK: patched {path}")
