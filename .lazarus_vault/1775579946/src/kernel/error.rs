use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    Critical, // CVSS 9.0-10.0
    High,     // CVSS 7.0-8.9
    Medium,   // CVSS 4.0-6.9
    Low,      // CVSS 0.1-3.9
}

#[derive(Debug, Clone)]
pub enum KernelError {
    VmoCloneFailed { parent: u32, size: usize },
    VmarMapFailed { vmar_handle: u32, size: usize },
    InvalidHandle(u32),
    AlignmentAnomaly { address: usize },
    SecurityViolation { message: String, cvss: f32 },
}

impl KernelError {
    pub fn severity(&self) -> Severity {
        match self {
            Self::SecurityViolation { cvss, .. } => {
                if *cvss >= 9.0 { Severity::Critical }
                else if *cvss >= 7.0 { Severity::High }
                else { Severity::Medium }
            },
            Self::VmarMapFailed { .. } => Severity::High,
            Self::VmoCloneFailed { .. } => Severity::High,
            Self::AlignmentAnomaly { .. } => Severity::Medium,
            Self::InvalidHandle(_) => Severity::Low,
        }
    }
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self.severity() {
            Severity::Critical => "![CRITICAL]!",
            Severity::High => "[HIGH]",
            Severity::Medium => "[MEDIUM]",
            Severity::Low => "[LOW]",
        };
        match self {
            Self::VmoCloneFailed { parent, size } => write!(f, "{} VMO_CLONE_FAIL: parent={} size={}", label, parent, size),
            Self::VmarMapFailed { vmar_handle, size } => write!(f, "{} VMAR_MAP_FAIL: handle=0x{:x} size={}", label, vmar_handle, size),
            Self::InvalidHandle(h) => write!(f, "{} INVALID_HANDLE: 0x{:x}", label, h),
            Self::AlignmentAnomaly { address } => write!(f, "{} ALIGN_FAIL: address=0x{:x}", label, address),
            Self::SecurityViolation { message, cvss } => write!(f, "{} SECURITY: {} (CVSS: {:.1})", label, message, cvss),
        }
    }
}

impl std::error::Error for KernelError {}
