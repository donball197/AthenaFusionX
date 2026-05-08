// bins/ide_backend/src/self_healing_engine.rs
// Re-exports the real engine from runtime_core.
pub use nexus_runtime_core::self_healing_engine::{
    HealingContext, HealingState, SelfHealingEngine,
};
pub fn init() {
    println!(">> [AIS] Self-Healing Engine module linked.");
}
