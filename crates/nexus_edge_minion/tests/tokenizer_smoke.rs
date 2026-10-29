use nexus_edge_minion::tokenizer::NexusTokenizer;
use std::path::PathBuf;

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

    let tok =
        NexusTokenizer::from_file(path.to_str().unwrap()).expect("Failed to load tokenizer.json");

    let text = "The capital of France is";
    let ids = tok.encode(text, true).expect("encode failed");
    println!(">> [TEST] \"{}\" -> {:?}", text, ids);
    assert!(!ids.is_empty(), "Encoding produced no tokens.");

    let decoded = tok.decode(&ids, true).expect("decode failed");
    println!(">> [TEST] decoded back -> \"{}\"", decoded);

    // Loose round-trip check -- tokenizers can add/normalize whitespace,
    // so we check substring containment rather than exact equality.
    assert!(
        decoded.to_lowercase().contains("capital") && decoded.to_lowercase().contains("france"),
        "Decoded text lost key content: got \"{}\"",
        decoded
    );

    println!(">> [PASS] Tokenizer round-trip smoke test passed successfully!");
}
