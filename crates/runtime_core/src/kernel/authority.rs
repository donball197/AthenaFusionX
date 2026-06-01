pub use crate::kernel::boundary::CapabilitySet;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct IdentityContext { pub actor_id: String, pub supervisor_id: String }

#[derive(Clone, Debug)]
pub struct CapabilityContext {
    pub bound_actor: String,
    pub capability_set: CapabilitySet,
    pub issued_by: String,
    pub expires_at: Option<SystemTime>,
}

#[derive(Clone, Debug)]
pub struct LineageContext { pub trace_id: String, pub parent_id: Option<String> }

#[derive(Clone, Debug)]
pub struct KernelRequest<T> {
    pub identity: IdentityContext,
    pub capability: CapabilityContext,
    pub lineage: LineageContext,
    pub payload: T,
}
