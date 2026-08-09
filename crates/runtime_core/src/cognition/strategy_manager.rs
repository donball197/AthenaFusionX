use super::traits::{Strategist, StrategyContext};
use async_trait::async_trait;

#[derive(Default, Clone)]
pub struct StrategyManager;

impl StrategyManager {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Strategist for StrategyManager {
    async fn select_strategy(&self, context: &StrategyContext) -> anyhow::Result<String> {
        let has_recent_failures = context.history.iter().any(|h| !h.failures.is_empty());
        
        let baseline = if context.goal.priority > 0.8 && context.available_resources < 0.5 {
            "High-priority constrained execution: bypass extensive audits."
        } else if has_recent_failures {
            "Conservative fallback: previous identical strategies yielded failures."
        } else {
            "Standard exploratory execution."
        };

        Ok(format!("{} Target: {}", baseline, context.goal.title))
    }
}
