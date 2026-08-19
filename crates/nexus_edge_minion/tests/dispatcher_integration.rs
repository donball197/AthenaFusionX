use nexus_edge_minion::dispatcher::HybridDispatcher;

#[test]
fn test_hybrid_dispatcher_flow_error_handling() {
    let dispatcher = HybridDispatcher::new(vec!["https://api.example.com/v1".to_string()]);
    let summary = "Layer 35 attention state stabilized.";
    let prompt = "Generate micro-agent execution plan.";

    // Test that an unreachable/dummy model correctly returns an Err safely instead of panicking.
    let response = dispatcher.process_task(summary, prompt, 64);
    
    assert!(response.is_err(), "Dispatcher should cleanly return an Err when network endpoints are unreachable.");
}
