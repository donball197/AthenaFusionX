//! Taint‑aware data flow – no arbitrary downgrade.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Taint violation: {0}")]
    TaintViolation(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaintLabel {
    Public = 0,
    Internal = 1,
    Secret = 2,
}

#[must_use = "Tainted values must be explicitly declassified via `flow_to`"]
#[derive(Debug, Clone)]
pub struct Tainted<T> {
    value: T,
    label: TaintLabel,
}

impl<T> Tainted<T> {
    pub fn new(value: T, label: TaintLabel) -> Self {
        Self { value, label }
    }

    pub fn flow_to(self, sink_level: TaintLabel) -> Result<T, SecurityError> {
        if self.label > sink_level {
            return Err(SecurityError::TaintViolation(format!(
                "Label {:?} cannot flow to sink level {:?}",
                self.label, sink_level
            )));
        }
        Ok(self.value)
    }

    pub fn map<U, F>(self, f: F) -> Tainted<U>
    where
        F: FnOnce(T) -> U,
    {
        Tainted {
            value: f(self.value),
            label: self.label,
        }
    }

    pub fn peek_label(&self) -> TaintLabel {
        self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_allowed() {
        let t = Tainted::new(42, TaintLabel::Internal);
        assert_eq!(t.flow_to(TaintLabel::Internal).unwrap(), 42);
    }

    #[test]
    fn test_flow_denied() {
        let t = Tainted::new("secret", TaintLabel::Secret);
        assert!(t.flow_to(TaintLabel::Public).is_err());
    }
}
