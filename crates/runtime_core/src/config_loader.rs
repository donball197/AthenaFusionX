use std::env;

pub struct Config {
    pub database_url: String,
    pub api_key: String,
}

pub fn validate_config() -> Config {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let api_key = env::var("API_KEY").expect("API_KEY must be set");
    
    Config { database_url, api_key }
}