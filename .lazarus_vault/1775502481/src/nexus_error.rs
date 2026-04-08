use std::fmt;
use crate::kernel::error::KernelError;

#[derive(Debug, Clone)]
pub enum NexusError {
    NotFound(String),
    Patch(String),
    KernelFault(KernelError),
    NeuralLinkSevered(String),
    Other(String),
    SecurityViolation { message: String, cvss: f32 },
}

impl fmt::Display for NexusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SecurityViolation { message, cvss } => write!(f, "!! [SECURITY] CVSS {:.1}: {}", cvss, message),
            Self::KernelFault(e) => write!(f, "!! [KERNEL_DNA_FAULT] {}", e),
            Self::NotFound(e) => write!(f, ">> [FS_FAULT] Not Found: {}", e),
            Self::Patch(e) => write!(f, ">> [DIFF_FAULT] Patch Failed: {}", e),
            Self::NeuralLinkSevered(e) => write!(f, "!! [UPLINK_FAULT]: {}", e),
            Self::Other(e) => write!(f, ">> [SYSTEM_FAULT]: {}", e),
        }
    }
}

impl std::error::Error for NexusError {}

impl From<std::io::Error> for NexusError {
    fn from(err: std::io::Error) -> Self {
        NexusError::Other(format!("IO_FAULT: {}", err))
    }
}

impl From<KernelError> for NexusError {
    fn from(err: KernelError) -> Self {
        NexusError::KernelFault(err)
    }
}
