path = "crates/nexus_edge_minion/tests/local_engine_smoke.rs"
with open(path) as f:
    src = f.read()

old = '''    let has_finite = hidden_state.iter().any(|&v| v.is_finite() && v != 0.0);
    assert!(has_finite, "Hidden state remained entirely zero or non-finite after forward step.");
    
    println!(">> [PASS] Local inference engine smoke test passed successfully!");
}'''

new = '''    let has_finite = hidden_state.iter().any(|&v| v.is_finite() && v != 0.0);
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
}'''

assert old in src, "Anchor not found -- aborting, no changes written"
src = src.replace(old, new, 1)
with open(path, "w") as f:
    f.write(src)
print("Applied test patch OK.")
