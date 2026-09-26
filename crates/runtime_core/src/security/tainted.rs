//! Taint-aware data flow - no arbitrary downgrade.
//!
//! The lattice lives in `nexus_substrate::taint` so the memory
//! substrate can use it without a circular dependency. This module
//! re-exports it and provides the `Tainted<T>` wrapper for call sites
//! that want the label attached to the value.

pub use nexus_substrate::taint::{
    Effect, FlowPolicy, Integrity, Secrecy, TaintError, TaintLabel,
};

#[must_use = "Tainted values must be explicitly declassified via `flow_to`"]
#[derive(Debug, Clone)]
pub struct Tainted<T> {
    value: T,
    label: TaintLabel,
}

impl<T> Tainted<T> {
    pub fn new(value: T, label: TaintLabel) -> Self { Self { value, label } }
    pub fn flow_to(self, policy: FlowPolicy) -> Result<T, TaintError> {
        self.label.flow_to(policy)?;
        Ok(self.value)
    }
    pub fn map<U, F>(self, f: F) -> Tainted<U> where F: FnOnce(T) -> U {
        Tainted { value: f(self.value), label: self.label }
    }
    pub fn peek_label(&self) -> TaintLabel { self.label }
    pub fn join_with(self, other: TaintLabel) -> Self {
        Self { value: self.value, label: self.label.join(other) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_allowed() {
        let t = Tainted::new(42, TaintLabel::INTERNAL);
        assert_eq!(t.flow_to(FlowPolicy::INTERNAL_SINK).unwrap(), 42);
    }

    #[test]
    fn flow_denied_integrity() {
        let t = Tainted::new("web", TaintLabel::UNTRUSTED_PUBLIC);
        assert!(t.flow_to(FlowPolicy::VERIFIED_SINK).is_err());
    }

    #[test]
    fn flow_denied_secrecy() {
        let t = Tainted::new("secret", TaintLabel::INTERNAL.with_secrecy(Secrecy::Secret));
        assert!(t.flow_to(FlowPolicy::PUBLIC_SINK).is_err());
    }
}
