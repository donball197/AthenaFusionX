use crate::config::NexusConfig;

pub struct ApiClient {
    config: NexusConfig,
}

impl ApiClient {
    pub fn new() -> Self {
        Self {
            config: NexusConfig::from_env(),
        }
    }

    /// Iterates automatically through the environment cascade configuration
    pub async fn dispatch_with_cascade(&self, prompt: &str, use_heavy: bool) -> Result<String, String> {
        let models = if use_heavy {
            &self.config.heavy_cascade
        } else {
            &self.config.light_cascade
        };

        if models.is_empty() {
            return Err("No models found in the active cascade configuration.".to_string());
        }

        let mut last_error = String::new();

        for model in models {
            println!("==> [ApiClient] Dispatching request to model: {}", model);
            
            match self.send_to_endpoint(model, prompt).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    eprintln!("==> [WARNING] Model '{}' failed/timed out: {}. Cascading to next target...", model, e);
                    last_error = e;
                }
            }
        }

        Err(format!("All models in cascade failed. Last error: {}", last_error))
    }

    async fn send_to_endpoint(&self, model: &str, _prompt: &str) -> Result<String, String> {
        // Wire your reqwest HTTP transport payload here, using the dynamic `model` string
        Ok(format!("Successfully evaluated via {}", model))
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}
