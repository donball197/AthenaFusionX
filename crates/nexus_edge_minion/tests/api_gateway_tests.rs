use nexus_edge_minion::api_gateway::{AgentRequest, ApiGateway};

#[test]
fn test_api_gateway_gemma4_dispatch() {
    let gateway = ApiGateway::new("https://api.example.com/v1", "test-key");
    let req = AgentRequest { prompt: "Test prompt".to_string(), temperature: 0.7, max_tokens: 64 };
    let res = gateway.dispatch(req).unwrap();
    assert!(res.text.contains("gemma-4-31-it"));
    assert_eq!(res.tokens_used, 64);
}
