#[derive(Debug)]
pub enum KernelError {
    VmoCloneFailed { parent: u32, size: usize },
    InvalidArgs,
    RegistryFull,
    AlignmentAnomaly { address: usize },
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for KernelError {}
