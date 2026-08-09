use nexus_edge_minion::inference_engine::InferenceEngine;

#[test]
fn test_inference_engine_invalid_path_handling() {
    let result = InferenceEngine::new("nonexistent_model.safetensors");
    assert!(result.is_err(), "Expected error when loading non-existent weights path.");
}
