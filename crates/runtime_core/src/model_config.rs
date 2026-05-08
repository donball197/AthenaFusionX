const DEFAULT_MODEL: &str = "models/gemma-4-31b-it";

pub fn get_model() -> String {
    std::env::var("NEXUS_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
}

pub fn get_embedding_model() -> String {
    std::env::var("NEXUS_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "models/gemini-embedding-2".to_string())
}
