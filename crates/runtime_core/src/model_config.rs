pub const DEFAULT_MODEL: &str = "gemma-4-31b-it";

pub fn get_model() -> String {
    std::env::var("NEXUS_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
}

pub fn get_embedding_model() -> String {
    std::env::var("NEXUS_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "models/gemini-embedding-2".to_string())
}

// ⚡ THE SOVEREIGN CASCADE: 31B Primary -> 3.1 Flash Failsafe -> 26B MoE Failsafe
pub fn get_light_models() -> Vec<String> { 
    vec![
        "gemini-3.1-flash-lite".to_string(), // Fast Primary
        "gemma-4-31b-it".to_string(),        // Failsafe 1
        "gemma-4-26b-a4b-it".to_string(),    // Failsafe 2
    ] 
}

pub fn get_heavy_models() -> Vec<String> { 
    vec![
        "gemma-4-31b-it".to_string(),        // Heavy Primary
        "gemini-3.1-flash-lite".to_string(), // Failsafe 1
        "gemma-4-26b-a4b-it".to_string(),    // Failsafe 2
    ] 
}
