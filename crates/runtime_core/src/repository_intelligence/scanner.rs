use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryScannerConfig {
    pub root_path: PathBuf,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub max_depth: usize,
    pub follow_symlinks: bool,
}

#[derive(Debug, Clone)]
pub struct Scanner {
    pub config: RepositoryScannerConfig,
    pub active_scans: usize,
    pub total_files_indexed: u64,
}

impl Scanner {
    pub fn new(config: RepositoryScannerConfig) -> Self {
        Self {
            config,
            active_scans: 0,
            total_files_indexed: 0,
        }
    }
}
