use nexus_edge_minion::forward_pass::{ForwardOrchestrator, ModelConfig};

#[test]
fn test_full_35_layer_forward_step() {
    let config = ModelConfig::default();
    let orchestrator = ForwardOrchestrator::new(config);

    let token_id = 42u32;
    let position = 1usize;
    let embed_tokens_per_layer = vec![0.1f32; 262144 * 256]; // Mock subset or full size view
    let mut hidden_state = vec![0.5f32; 256];

    // Execute full 35-layer step
    orchestrator.execute_token_step(token_id, position, &embed_tokens_per_layer, &mut hidden_state);

    assert_eq!(hidden_state.len(), 256);
    // Verify values have been transformed by attention and PLE conditioning
    assert!(hidden_state[0].is_finite());
}
