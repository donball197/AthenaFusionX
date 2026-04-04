use crate::resonator::ResonatorCore;
use anyhow::Result;

pub async fn run_logic(input: &str) -> Result<String> {
    Ok(format!(">> [REFLEX] Local processing of: {}", input))
}

pub fn ignite_agent(path: &str) -> Result<()> {
    ResonatorCore::ignite_vault(path).map(|_| ())
}
