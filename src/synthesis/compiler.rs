use crate::synthesis::policy::SovereignPolicy;
use crate::governance::axioms::AxiomSentinel;
use crate::wasm_registry::WasmRegistry;
use tracing::info;

pub struct PolicyCompiler;

impl PolicyCompiler {
    pub async fn mount_policy(
        policy: SovereignPolicy, 
        registry: &WasmRegistry,
        _architect_sig: &str 
    ) -> Result<(), String> {
        // ⚡ V18 Safety Gate
        AxiomSentinel::verify_intent(&policy.code)?;

        info!(id = %policy.id, "Axioms verified. Mounting policy to registry.");

        registry.store(&policy.id, &policy.code, &policy.objective)
            .await
            .map_err(|e| format!("Registry anchoring failed: {}", e))?;

        Ok(())
    }
}
