//! NEXUS Algorithms
//!
//! Pure capability catalog and model-tier routing.
//!
//! This crate deliberately has no dependency on runtime_core and performs
//! no I/O, environment access, execution, authorization, or tool dispatch.
//!
//! `capability` answers:
//!   "What cognitive capabilities exist and what do they represent?"
//!
//! `routing` answers:
//!   "Which model tier serves a model-backed capability?"
//!
//! The actual implementations remain in their independent algorithm crates
//! and runtime engines.

pub mod capability;
pub mod routing;

pub use capability::{CapabilityDescriptor, CapabilityKind, CapabilityRegistry};
pub use routing::{ModelTier, Router};
