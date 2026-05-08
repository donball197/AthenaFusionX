// bins/ide_backend/src/lazarus_vault.rs
// Re-exports the vault from runtime_core.
pub use nexus_runtime_core::lazarus_vault::{LazarusVault, Snapshot};
pub fn init() {
    println!(">> [LAZARUS] Vault module linked.");
}
