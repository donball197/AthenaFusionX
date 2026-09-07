//! Layered In-Memory Copy-on-Write Virtual File System for AthenaFusionX.
//!
//! Provides isolated dry-run environments for AST remediation and agent mutations.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    Unmodified(Vec<u8>),
    Modified(Vec<u8>),
    Deleted,
    Created(Vec<u8>),
}

#[derive(Debug, Clone, Default)]
pub struct VirtualFS {
    base_dir: PathBuf,
    overlay: HashMap<PathBuf, FileState>,
}

impl VirtualFS {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self { base_dir: base_dir.as_ref().to_path_buf(), overlay: HashMap::new() }
    }

    fn normalize_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let p = path.as_ref();
        if p.is_absolute() {
            if let Ok(rel) = p.strip_prefix(&self.base_dir) {
                rel.to_path_buf()
            } else {
                p.to_path_buf()
            }
        } else {
            p.to_path_buf()
        }
    }

    pub fn read_to_string(&mut self, path: impl AsRef<Path>) -> Result<String> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).map_err(|e| anyhow!("UTF-8 decode error: {}", e))
    }

    pub fn read(&mut self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        let norm = self.normalize_path(path);
        if let Some(state) = self.overlay.get(&norm) {
            return match state {
                FileState::Unmodified(bytes)
                | FileState::Modified(bytes)
                | FileState::Created(bytes) => Ok(bytes.clone()),
                FileState::Deleted => {
                    Err(anyhow!("File '{:?}' has been deleted in VFS overlay", norm))
                }
            };
        }

        let physical_path = self.base_dir.join(&norm);
        if !physical_path.exists() {
            return Err(anyhow!("File '{:?}' not found on physical disk or VFS", norm));
        }

        let bytes = fs::read(&physical_path)?;
        self.overlay.insert(norm, FileState::Unmodified(bytes.clone()));
        Ok(bytes)
    }

    pub fn write(&mut self, path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<()> {
        let norm = self.normalize_path(path);
        let bytes = content.as_ref().to_vec();

        if let Some(FileState::Created(_)) = self.overlay.get(&norm) {
            self.overlay.insert(norm, FileState::Created(bytes));
        } else {
            let physical_path = self.base_dir.join(&norm);
            if physical_path.exists() {
                self.overlay.insert(norm, FileState::Modified(bytes));
            } else {
                self.overlay.insert(norm, FileState::Created(bytes));
            }
        }
        Ok(())
    }

    pub fn delete(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let norm = self.normalize_path(path);
        self.overlay.insert(norm, FileState::Deleted);
        Ok(())
    }

    pub fn has_mutations(&self) -> bool {
        self.overlay.values().any(|s| !matches!(s, FileState::Unmodified(_)))
    }

    pub fn dirty_files(&self) -> Vec<PathBuf> {
        self.overlay
            .iter()
            .filter_map(|(p, s)| match s {
                FileState::Unmodified(_) => None,
                _ => Some(p.clone()),
            })
            .collect()
    }

    pub fn commit_to_disk(&mut self) -> Result<usize> {
        let mut committed = 0;
        for (rel_path, state) in &self.overlay {
            let target = self.base_dir.join(rel_path);
            match state {
                FileState::Modified(bytes) | FileState::Created(bytes) => {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&target, bytes)?;
                    committed += 1;
                }
                FileState::Deleted => {
                    if target.exists() {
                        fs::remove_file(&target)?;
                        committed += 1;
                    }
                }
                FileState::Unmodified(_) => {}
            }
        }
        self.overlay.clear();
        Ok(committed)
    }

    pub fn discard(&mut self) {
        self.overlay.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_vfs_cow_lifecycle() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("code.py");
        fs::write(&file_path, "print('original')")?;

        let mut vfs = VirtualFS::new(dir.path());

        // Read physical
        let content = vfs.read_to_string("code.py")?;
        assert_eq!(content, "print('original')");

        // Write overlay
        vfs.write("code.py", "print('healed')")?;
        assert!(vfs.has_mutations());
        assert_eq!(vfs.read_to_string("code.py")?, "print('healed')");

        // Physical disk is untouched
        assert_eq!(fs::read_to_string(&file_path)?, "print('original')");

        // Commit overlay to disk
        let committed = vfs.commit_to_disk()?;
        assert_eq!(committed, 1);
        assert_eq!(fs::read_to_string(&file_path)?, "print('healed')");
        assert!(!vfs.has_mutations());

        Ok(())
    }
}
