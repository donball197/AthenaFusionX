const DEFAULT_MODEL: &str = "gemma-4-31b-it";

pub fn get_model() -> String {
    std::env::var("NEXUS_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
}

pub fn get_embedding_model() -> String {
    std::env::var("NEXUS_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "models/gemini-embedding-2".to_string())
}

pub fn get_light_models() -> Vec<String> {
    vec!["gemini-3.1-flash-lite".to_string(), "gemma-4-26b-a4b-it".to_string()]
}

pub fn get_heavy_models() -> Vec<String> {
    vec![
        "gemma-4-31b-it".to_string(),
        "gemini-3.1-flash-lite".to_string(),
        "gemma-4-26b-a4b-it".to_string(),
    ]
}
