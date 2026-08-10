use nexus_edge_minion::inference_engine::InferenceEngine;
use std::path::Path;

#[test]
fn test_local_engine_safetensors_smoke() {
    let weights_path = Path::new("gemma-4-E2B-it.safetensors");
    
    // If the weights file isn't present yet, skip gracefully instead of failing CI/test suites
    if !weights_path.exists() {
        println!(">> [SKIP] gemma-4-E2B-it.safetensors not found in root. Skipping local weights smoke test.");
        return;
    }

    println!(">> [TEST] Initializing InferenceEngine with local safetensors...");
    let mut engine = match InferenceEngine::new(weights_path) {
        Ok(eng) => eng,
        Err(e) => {
            panic!("FATAL: Failed to initialize InferenceEngine: {}", e);
        }
    };

    println!(">> [TEST] Engine online. Executing single token warmup step at position 0...");
    let mut hidden_state = vec![0.0f32; 1536];
    let result = engine.step(42, 0, &mut hidden_state);

    assert!(result.is_ok(), "Token execution step failed.");
    
    // Verify the hidden state was mutated by the layers and contains valid finite numbers
    let has_finite = hidden_state.iter().any(|&v| v.is_finite() && v != 0.0);
    assert!(has_finite, "Hidden state remained entirely zero or non-finite after forward step.");

    println!(">> [TEST] Computing logits via tied LM head + softcap...");
    let logits = engine.compute_logits(&hidden_state).expect("compute_logits failed");
    assert_eq!(logits.len(), 262144, "Unexpected vocab size in logits.");
    assert!(logits.iter().all(|v| v.is_finite()), "Non-finite value in logits.");

    let mut indexed: Vec<(usize, f32)> = logits.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    println!(">> [TEST] Top-5 tokens by logit:");
    for (rank, (tok_id, val)) in indexed.iter().take(5).enumerate() {
        println!("    #{} token_id={} logit={:.4}", rank + 1, tok_id, val);
    }

    let (argmax_id, argmax_val) = indexed[0];
    assert!(argmax_id < 262144, "Argmax token id out of vocab range.");
    println!(">> [TEST] Argmax token_id={} logit={:.4}", argmax_id, argmax_val);

    println!(">> [PASS] Local inference engine smoke test passed successfully!");
}
