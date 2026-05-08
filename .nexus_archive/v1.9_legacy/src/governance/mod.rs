// src/governance/mod.rs — v1.1 [COMPLETE_MODULE_TREE]
// FIX: wal and heartbeat existed on disk but were not declared here,
//      making them unreachable via the module tree.
pub mod axioms;
pub mod controller;
pub mod heartbeat;
pub mod protocol;
pub mod wal;
