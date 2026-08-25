use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fs;
use std::sync::RwLock;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PromptConfig {
    pub sovereign_persona: String,
}

static PROMPT_CONFIG: Lazy<RwLock<PromptConfig>> = Lazy::new(|| {
    let config = load_config().unwrap_or_default();
    RwLock::new(config)
});

fn load_config() -> Result<PromptConfig, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string("config/prompts.yaml")?;
    let config: PromptConfig = serde_yaml::from_str(&contents)?;
    Ok(config)
}

pub fn get_sovereign_persona() -> String {
    PROMPT_CONFIG.read().unwrap().sovereign_persona.clone()
}

pub fn reload_prompts() -> Result<(), Box<dyn std::error::Error>> {
    let new_config = load_config()?;
    let mut config = PROMPT_CONFIG.write().unwrap();
    *config = new_config;
    Ok(())
}
