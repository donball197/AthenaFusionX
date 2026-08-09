pub mod factory;
pub mod neural_agent;

use async_trait::async_trait;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn execute(&self, prompt: &str) -> Result<String, String>;
    fn get_model_name(&self) -> &str;
}
pub mod universal_provider;
