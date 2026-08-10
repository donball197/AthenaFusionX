path = "crates/nexus_edge_minion/tests/tokenizer_smoke.rs"
with open(path) as f:
    src = f.read()

old = '''use nexus_edge_minion::tokenizer::NexusTokenizer;
use std::path::Path;

#[test]
fn test_tokenizer_roundtrip() {
    let path = Path::new("tokenizer.json");
    if !path.exists() {
        println!(">> [SKIP] tokenizer.json not found in root. Skipping tokenizer smoke test.");
        return;
    }

    let tok = NexusTokenizer::from_file(path.to_str().unwrap())
        .expect("Failed to load tokenizer.json");'''

new = '''use nexus_edge_minion::tokenizer::NexusTokenizer;
use std::path::{Path, PathBuf};

fn find_tokenizer_json() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("tokenizer.json"),
        manifest_dir.join("../../tokenizer.json"),
        manifest_dir.join("../tokenizer.json"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

#[test]
fn test_tokenizer_roundtrip() {
    let path = match find_tokenizer_json() {
        Some(p) => p,
        None => {
            println!(">> [SKIP] tokenizer.json not found in any candidate location. Skipping tokenizer smoke test.");
            return;
        }
    };
    println!(">> [TEST] Using tokenizer.json at: {}", path.display());

    let tok = NexusTokenizer::from_file(path.to_str().unwrap())
        .expect("Failed to load tokenizer.json");'''

assert old in src, "Anchor not found -- aborting, no changes written"
src = src.replace(old, new, 1)
with open(path, "w") as f:
    f.write(src)
print("Patched tokenizer path resolution OK.")
