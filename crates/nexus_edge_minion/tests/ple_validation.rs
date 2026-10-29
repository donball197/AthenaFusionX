use nexus_edge_minion::ple::{Stage1PLE, Stage2PLE};

#[test]
fn test_35_layer_pipeline_validation() {
    let num_layers = 35;
    let layer_dim = 256;
    let total_dim = num_layers * layer_dim;

    let ple1 = Stage1PLE::new(num_layers, layer_dim);
    let raw_ple = vec![0.1f32; total_dim];
    let proj = vec![0.2f32; total_dim];
    let mut blended = vec![0.0f32; total_dim];

    // Stage 1 blend
    ple1.blend_stage1(&raw_ple, &proj, &mut blended);
    assert!((blended[0] - 0.212132).abs() < 1e-4);

    // Stage 2 layer pass across all 35 layers
    let residual = vec![0.5f32; layer_dim];
    let mut layer_states = vec![0.3f32; layer_dim];

    for _layer in 0..num_layers {
        Stage2PLE::forward_layer(&mut layer_states, &residual, 1.0);
    }

    assert_eq!(layer_states.len(), layer_dim);
}
