pub trait ResourceAwareRouter {
    fn route_request(&self, model_size: usize, node_arch: &str) -> String {
        if node_arch == "arm64" && model_size > 1024 {
            "fallback_to_heavy_node".to_string()
        } else {
            "action_b_lightweight".to_string()
        }
    }
}
