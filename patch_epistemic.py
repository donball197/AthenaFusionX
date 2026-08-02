import os

path = "crates/runtime_core/src/epistemic_engine.rs"
if not os.path.exists(path):
    print(f"Error: {path} not found. Please run from ~/AthenaFusionX.")
    exit(1)

with open(path, "r") as f:
    content = f.read()

sanitizer = """
fn sanitize_json_escapes(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next == '"' || next == '\\' || next == '/' || next == 'b' || next == 'f' || next == 'n' || next == 'r' || next == 't' || next == 'u' {
                    out.push('\\');
                } else {
                    out.push_str("\\\\");
                }
            } else {
                out.push_str("\\\\");
            }
        } else {
            out.push(c);
        }
    }
    out
}
"""

if "sanitize_json_escapes" not in content:
    content = sanitizer + "\n" + content
    with open(path, "w") as f:
        f.write(content)
    print(f"[OK] Patched {path}")
else:
    print(f"[SKIP] {path} already contains sanitize_json_escapes")
