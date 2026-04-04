use ax_auth::NexusIdentity;
use crate::nexus_error::NexusError;

pub async fn verify_sovereign_link(token: &str) -> Result<bool, NexusError> {
    // Simple local key verification to ensure only the Lead Architect 
    // can uplink to the Kernel on the local network.
    let key = std::env::var("NEXUS_AUTH_KEY").unwrap_or_else(|_| "SOVEREIGN".into());
    Ok(token == key)
}
