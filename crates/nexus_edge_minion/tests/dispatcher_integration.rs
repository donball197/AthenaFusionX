use nexus_edge_minion::dispatcher::HybridDispatcher;

#[test]
fn test_hybrid_dispatcher_flow() {
    let dispatcher = HybridDispatcher::new("https://api.example.com/v1", "test-key");
    let summary = "Layer 35 attention state stabilized.";
    let prompt = "Generate micro-agent execution plan.";

    let response = dispatcher.process_task(summary, prompt, 64).unwrap();
    assert!(response.text.contains("gemma-4-31-it"));
    assert!(response.text.contains("Layer 35 attention state stabilized"));
}
