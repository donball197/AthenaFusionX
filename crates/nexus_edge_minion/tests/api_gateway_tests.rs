use nexus_edge_minion::api_gateway::{AgentRequest, ApiGateway};

#[test]
fn test_api_gateway_error_cascade() {
    let gateway = ApiGateway::new();
    let req = AgentRequest { prompt: "Test prompt".to_string(), temperature: 0.7, max_tokens: 64 };

    // Test that an unreachable/dummy model correctly exhausts the cascade
    // and returns an Err safely instead of panicking.
    let res = gateway.dispatch_cascade(req, &["dummy_model".to_string()]);

    assert!(res.is_err(), "Gateway should cleanly return an Err when all models fail.");
}
