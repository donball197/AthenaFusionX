import sys

path = "apps/nexus_ast_tool/src/main.rs"

old = '''fn contains_safe_pattern(pattern_name: &str, patched_source: &str) -> bool {
    let lower = patched_source.to_lowercase();
    match pattern_name {
        "SQL_INJECTION" => {
            lower.contains("mysqli_prepare") 
            || lower.contains("bind_param") 
            || lower.contains("pdo::prepare")
            || lower.contains("execute(array")
            || lower.contains("pg_prepare")
        },
        "XSS" => {
            lower.contains("htmlspecialchars") 
            || lower.contains("htmlentities")
            || lower.contains("strip_tags")
        },
        _ => false
    }
}'''

new = '''fn contains_safe_pattern(pattern_name: &str, patched_source: &str) -> bool {
    let lower = patched_source.to_lowercase();

    // Obvious hallucinated stubs must never pass, whatever pattern_name is.
    let stub_markers = ["your_secured_query_here", "todo", "insert_your_query", "placeholder", "fixme"];
    if stub_markers.iter().any(|m| lower.contains(m)) {
        return false;
    }

    match pattern_name {
        "SQL_INJECTION" => {
            // A bare mysqli_prepare($con, $q) proves nothing on its own: if $q
            // still holds a directly-concatenated string (no '?' placeholder),
            // prepare()+execute() runs the exact same tainted query mysqli_query()
            // would have -- it's a cosmetic wrapper, not a fix. Require actual
            // evidence of parameterization: a literal '?' placeholder together
            // with a real bind call, or PDO's execute(array(...)).
            let has_placeholder = patched_source.contains('?');
            let has_bind = lower.contains("bind_param")
                || lower.contains("bindvalue")
                || lower.contains("bindparam")
                || lower.contains("execute(array");
            (has_placeholder && has_bind) || lower.contains("pg_prepare")
        },
        "XSS" => {
            lower.contains("htmlspecialchars") 
            || lower.contains("htmlentities")
            || lower.contains("strip_tags")
        },
        _ => false
    }
}'''

with open(path, "r") as f:
    content = f.read()

count = content.count(old)
if count != 1:
    print(f"ANCHOR MISMATCH: found {count} occurrences, expected 1", file=sys.stderr)
    sys.exit(1)

with open(path, "w") as f:
    f.write(content.replace(old, new))

print("OK: contains_safe_pattern now requires real placeholder+bind evidence, rejects stub markers")
