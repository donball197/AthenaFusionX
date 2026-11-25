//! Held remediation: verified patches persisted for explicit human promotion.
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_DIR: &str = ".nexus/holds";

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum FileStatus {
    Held,
    Promoted,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeldFinding {
    pub pattern: String,
    pub line: usize,
    pub patch: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeldFile {
    pub rel: String,
    pub status: FileStatus,
    pub findings: Vec<HeldFinding>,
    #[serde(default)]
    pub orig_sha256: Option<String>,
    #[serde(default)]
    pub patched_sha256: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Hold {
    pub id: String,
    pub created_unix: u64,
    pub authoritative_root: PathBuf,
    pub single_file: bool,
    pub files: Vec<HeldFile>,
}

fn safe_rel(rel: &str) -> Result<PathBuf> {
    let p = PathBuf::from(rel);
    if p.as_os_str().is_empty() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        bail!("unsafe relative path in hold: {rel}");
    }
    Ok(p)
}

fn safe_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("invalid hold id: {id}");
    }
    Ok(())
}


fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn require_regular_non_symlink(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("cannot inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("expected a regular non-symlink file: {}", path.display());
    }
    Ok(())
}

fn require_directory_non_symlink(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("cannot inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("expected a regular non-symlink directory: {}", path.display());
    }
    Ok(())
}

fn checked_target(h: &Hold, rel: &Path) -> Result<PathBuf> {
    let root_meta = fs::symlink_metadata(&h.authoritative_root)
        .with_context(|| format!("cannot inspect {}", h.authoritative_root.display()))?;
    if root_meta.file_type().is_symlink() {
        bail!("authoritative root must not be a symlink");
    }
    let root = h.authoritative_root.canonicalize()?;
    if h.single_file {
        if !root_meta.is_file() {
            bail!("single-file authoritative root is not a regular file");
        }
        return Ok(root);
    }
    if !root_meta.is_dir() {
        bail!("authoritative root is not a directory");
    }
    let mut current = root.clone();
    for component in rel.components() {
        let Component::Normal(part) = component else {
            bail!("unsafe path component in target");
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("cannot inspect {}", current.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("symlink in authoritative target path: {}", current.display());
        }
    }
    require_regular_non_symlink(&current)?;
    let canonical = current.canonicalize()?;
    if !canonical.starts_with(&root) {
        bail!("target escapes authoritative root");
    }
    Ok(canonical)
}

fn checked_artifact(dir: &Path, id: &str, kind: &str, rel: &Path) -> Result<PathBuf> {
    let base = dir.join(id);
    require_directory_non_symlink(&base)?;
    let artifact_root = base.join(kind);
    require_directory_non_symlink(&artifact_root)?;
    let mut current = artifact_root;
    for component in rel.components() {
        let Component::Normal(part) = component else {
            bail!("unsafe artifact path");
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("cannot inspect artifact {}", current.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("symlink in hold artifact path: {}", current.display());
        }
    }
    require_regular_non_symlink(&current)?;
    Ok(current)
}

fn write_atomic(target: &Path, bytes: &[u8], permissions: Option<fs::Permissions>) -> Result<()> {
    let parent = target.parent().ok_or_else(|| anyhow!("no parent"))?;
    let name = target.file_name().ok_or_else(|| anyhow!("no file name"))?.to_string_lossy();
    let mut temp_path = None;
    let mut temp_file: Option<File> = None;
    for _ in 0..100 {
        let nonce = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{name}.nexus-{}-{nonce}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(file) => { temp_path = Some(candidate); temp_file = Some(file); break; }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e).context("cannot create temp file"),
        }
    }
    let temp_path = temp_path.ok_or_else(|| anyhow!("could not allocate temp file"))?;
    let mut file = temp_file.unwrap();
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        if let Some(p) = permissions { file.set_permissions(p)?; }
        drop(file);
        fs::rename(&temp_path, target)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp_path); }
    result
}

fn validate_manifest(h: &Hold, id: &str) -> Result<()> {
    if h.id != id { bail!("hold manifest ID mismatch"); }
    let mut seen = HashSet::new();
    for file in &h.files {
        let rel = safe_rel(&file.rel)?;
        if !seen.insert(rel) { bail!("duplicate file path in hold: {}", file.rel); }
    }
    Ok(())
}

fn save(dir: &Path, h: &Hold) -> Result<()> {
    let p = dir.join(&h.id).join("hold.json");
    let tmp = dir.join(&h.id).join("hold.json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(h)?)?;
    fs::rename(tmp, p)?;
    Ok(())
}

pub fn load(dir: &Path, id: &str) -> Result<Hold> {
    safe_id(id)?;
    let data = fs::read_to_string(dir.join(id).join("hold.json"))
        .map_err(|e| anyhow!("cannot read hold {id}: {e}"))?;
    Ok(serde_json::from_str(&data)?)
}

fn target(h: &Hold, rel: &Path) -> PathBuf {
    if h.single_file { h.authoritative_root.clone() } else { h.authoritative_root.join(rel) }
}

/// entries: (authoritative file, sandbox file, findings fixed in it)
pub fn create(
    dir: &Path,
    auth_root: &Path,
    entries: Vec<(PathBuf, PathBuf, Vec<HeldFinding>)>,
) -> Result<Hold> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let id = format!("hold_{}_{}", ts, std::process::id());
    let single = auth_root.is_file();
    let base = dir.join(&id);
    let mut files = Vec::new();
    for (auth, sb, findings) in entries {
        let rel: PathBuf = if single {
            PathBuf::from(auth.file_name().ok_or_else(|| anyhow!("no file name"))?)
        } else {
            let abs = auth.canonicalize().unwrap_or(auth.clone());
            abs.strip_prefix(auth_root)
                .map_err(|_| anyhow!("{} outside {}", abs.display(), auth_root.display()))?
                .to_path_buf()
        };
        let rel_s = rel.to_string_lossy().to_string();
        safe_rel(&rel_s)?;
        let orig_bytes = fs::read(&auth)
            .with_context(|| format!("cannot read original {}", auth.display()))?;
        let patched_bytes = fs::read(&sb)
            .with_context(|| format!("cannot read patched {}", sb.display()))?;

        for (sub, bytes) in [("patched", &patched_bytes), ("orig", &orig_bytes)] {
            let dst = base.join(sub).join(&rel);
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p)?;
            }
            fs::write(&dst, bytes)?;
        }
        files.push(HeldFile {
            rel: rel_s,
            status: FileStatus::Held,
            findings,
            orig_sha256: Some(sha256(&orig_bytes)),
            patched_sha256: Some(sha256(&patched_bytes)),
        });
    }
    let h = Hold {
        id,
        created_unix: ts,
        authoritative_root: auth_root.to_path_buf(),
        single_file: single,
        files,
    };
    save(dir, &h)?;
    Ok(h)
}

pub fn list(dir: &Path) -> Result<()> {
    let Ok(rd) = fs::read_dir(dir) else {
        println!("No holds in {}", dir.display());
        return Ok(());
    };
    for e in rd.flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        if let Ok(h) = load(dir, &id) {
            let held = h.files.iter().filter(|f| f.status == FileStatus::Held).count();
            println!("{}  files={} held={} root={}", h.id, h.files.len(), held, h.authoritative_root.display());
        }
    }
    Ok(())
}

pub fn show(dir: &Path, id: &str) -> Result<()> {
    let h = load(dir, id)?;
    println!("Hold {} (root {})", h.id, h.authoritative_root.display());
    for f in &h.files {
        println!("\n  [{:?}] {}", f.status, f.rel);
        for x in &f.findings {
            println!("    - {} @ line {}", x.pattern, x.line);
            for l in x.patch.lines().take(8) {
                println!("        | {l}");
            }
        }
    }
    Ok(())
}

/// Returns (promoted, conflicts). Requires interactive TTY + typed confirmation.
pub fn promote(dir: &Path, id: &str) -> Result<(usize, usize)> {
    if !std::io::stdin().is_terminal() {
        bail!("promote requires an interactive terminal (stdin is not a TTY)");
    }
    show(dir, id)?;
    print!("\nType PROMOTE {} to apply held patches: ", id);
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    if line.trim() != format!("PROMOTE {}", id) {
        bail!("confirmation mismatch; nothing written");
    }
    promote_confirmed(dir, id)
}

/// Non-interactive promote. Caller has already verified confirmation
/// (e.g. the IDE backend checked `confirmation == "PROMOTE <id>"`).
/// Returns `(promoted_count, conflict_count)`.
pub fn promote_confirmed(dir: &Path, id: &str) -> Result<(usize, usize)> {
    let mut h = load(dir, id)?;
    validate_manifest(&h, id)?;
    let (mut promoted, mut conflicts) = (0usize, 0usize);

    for i in 0..h.files.len() {
        if h.files[i].status != FileStatus::Held {
            continue;
        }

        let rel = safe_rel(&h.files[i].rel)?;

        let orig_hash = h.files[i].orig_sha256.as_deref()
            .ok_or_else(|| anyhow!("hold {} has no original hash; recreate it", id))?;
        let patched_hash = h.files[i].patched_sha256.as_deref()
            .ok_or_else(|| anyhow!("hold {} has no patched hash; recreate it", id))?;

        let orig_path = checked_artifact(dir, id, "orig", &rel)?;
        let patched_path = checked_artifact(dir, id, "patched", &rel)?;

        let orig = fs::read(&orig_path)?;
        let patched = fs::read(&patched_path)?;

        if sha256(&orig) != orig_hash {
            bail!("original artifact integrity check failed: {}", rel.display());
        }
        if sha256(&patched) != patched_hash {
            bail!("patched artifact integrity check failed: {}", rel.display());
        }

        let tgt = checked_target(&h, &rel)?;
        let current = fs::read(&tgt)
            .with_context(|| format!("cannot read target {}", tgt.display()))?;

        if current != orig {
            eprintln!("CONFLICT {}: changed since hold; skipped", tgt.display());
            conflicts += 1;
            continue;
        }

        let metadata = fs::metadata(&tgt)?;
        let original_status = h.files[i].status.clone();

        write_atomic(&tgt, &patched, Some(metadata.permissions()))?;

        h.files[i].status = FileStatus::Promoted;

        if let Err(save_error) = save(dir, &h) {
            h.files[i].status = original_status;
            let rollback = write_atomic(&tgt, &orig, Some(metadata.permissions()));
            if let Err(rollback_error) = rollback {
                bail!(
                    "manifest save failed ({save_error}); rollback also failed ({rollback_error}); target may need manual recovery: {}",
                    tgt.display()
                );
            }
            return Err(save_error).context("manifest save failed; original restored");
        }

        promoted += 1;
    }

    Ok((promoted, conflicts))
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nexus_hold_test_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn setup_files(base: &Path) -> (PathBuf, PathBuf) {
        let orig = base.join("orig_target.php");
        let patched = base.join("patched_target.php");
        fs::write(&orig, "<?php echo 'vulnerable'; ?>").unwrap();
        fs::write(&patched, "<?php echo 'safe'; ?>").unwrap();
        (orig, patched)
    }

    #[test]
    fn create_and_load_roundtrip() {
        let hold_dir = tmp_dir("roundtrip");
        let work = tmp_dir("roundtrip_work");
        let (orig, patched) = setup_files(&work);

        let findings = vec![HeldFinding {
            pattern: "SQL_INJECTION".into(),
            line: 2,
            patch: "- vulnerable\n+ safe".into(),
        }];

        let h = create(&hold_dir, &work, vec![(orig.clone(), patched.clone(), findings)])
            .expect("create must succeed");

        assert!(h.id.starts_with("hold_"));
        assert_eq!(h.files.len(), 1);
        assert_eq!(h.files[0].status, FileStatus::Held);
        assert_eq!(h.files[0].findings.len(), 1);
        assert!(h.files[0].orig_sha256.is_some());
        assert!(h.files[0].patched_sha256.is_some());

        let loaded = load(&hold_dir, &h.id).expect("load must succeed");
        assert_eq!(loaded.id, h.id);
        assert_eq!(loaded.files[0].rel, h.files[0].rel);

        let _ = fs::remove_dir_all(&hold_dir);
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn promote_confirmed_applies_patch() {
        let hold_dir = tmp_dir("promote");
        let work = tmp_dir("promote_work");
        let (orig, patched) = setup_files(&work);

        let findings = vec![HeldFinding {
            pattern: "SQL_INJECTION".into(),
            line: 2,
            patch: "- vulnerable\n+ safe".into(),
        }];

        let h = create(&hold_dir, &work, vec![(orig.clone(), patched.clone(), findings)])
            .expect("create");

        let (promoted, conflicts) = promote_confirmed(&hold_dir, &h.id).expect("promote");
        assert_eq!(promoted, 1);
        assert_eq!(conflicts, 0);

        // Target should now have patched content
        let content = fs::read_to_string(&orig).unwrap();
        assert_eq!(content, "<?php echo 'safe'; ?>");

        let _ = fs::remove_dir_all(&hold_dir);
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn promote_confirmed_detects_conflict() {
        let hold_dir = tmp_dir("conflict");
        let work = tmp_dir("conflict_work");
        let (orig, patched) = setup_files(&work);

        let findings = vec![HeldFinding {
            pattern: "SQL_INJECTION".into(),
            line: 2,
            patch: "- vulnerable\n+ safe".into(),
        }];

        let h = create(&hold_dir, &work, vec![(orig.clone(), patched.clone(), findings)])
            .expect("create");

        // Modify the target AFTER the hold is created
        fs::write(&orig, "<?php echo 'CHANGED_EXTERNALLY'; ?>").unwrap();

        let (promoted, conflicts) = promote_confirmed(&hold_dir, &h.id).expect("promote");
        assert_eq!(promoted, 0);
        assert_eq!(conflicts, 1);

        // Target should NOT have been modified
        let content = fs::read_to_string(&orig).unwrap();
        assert_eq!(content, "<?php echo 'CHANGED_EXTERNALLY'; ?>");

        let _ = fs::remove_dir_all(&hold_dir);
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn promote_confirmed_skips_already_promoted() {
        let hold_dir = tmp_dir("skip");
        let work = tmp_dir("skip_work");
        let (orig, patched) = setup_files(&work);

        let findings = vec![HeldFinding {
            pattern: "SQL_INJECTION".into(),
            line: 2,
            patch: "- vulnerable\n+ safe".into(),
        }];

        let h = create(&hold_dir, &work, vec![(orig.clone(), patched.clone(), findings)])
            .expect("create");

        // First promote
        let (p1, c1) = promote_confirmed(&hold_dir, &h.id).expect("promote 1");
        assert_eq!(p1, 1);
        assert_eq!(c1, 0);

        // Second promote — should skip (already Promoted)
        let (p2, c2) = promote_confirmed(&hold_dir, &h.id).expect("promote 2");
        assert_eq!(p2, 0);
        assert_eq!(c2, 0);

        let _ = fs::remove_dir_all(&hold_dir);
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn promote_confirmed_rejects_tampered_artifact() {
        let hold_dir = tmp_dir("tamper");
        let work = tmp_dir("tamper_work");
        let (orig, patched) = setup_files(&work);

        let findings = vec![HeldFinding {
            pattern: "SQL_INJECTION".into(),
            line: 2,
            patch: "- vulnerable\n+ safe".into(),
        }];

        let h = create(&hold_dir, &work, vec![(orig.clone(), patched.clone(), findings)])
            .expect("create");

        // Tamper with the patched artifact AFTER hold creation
        let artifact_path = hold_dir.join(&h.id).join("patched").join("orig_target.php");
        fs::write(&artifact_path, "<?php echo 'EVIL'; ?>").unwrap();

        let result = promote_confirmed(&hold_dir, &h.id);
        assert!(result.is_err(), "must reject tampered artifact");

        // Target should be unchanged
        let content = fs::read_to_string(&orig).unwrap();
        assert_eq!(content, "<?php echo 'vulnerable'; ?>");

        let _ = fs::remove_dir_all(&hold_dir);
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn load_rejects_wrong_id() {
        let hold_dir = tmp_dir("wrongid");
        let work = tmp_dir("wrongid_work");
        let (orig, patched) = setup_files(&work);

        let findings = vec![HeldFinding {
            pattern: "SQL_INJECTION".into(),
            line: 2,
            patch: "- vulnerable\n+ safe".into(),
        }];

        let h = create(&hold_dir, &work, vec![(orig, patched, findings)]).expect("create");

        let result = load(&hold_dir, "wrong_id_999");
        assert!(result.is_err(), "load must reject wrong ID");

        let _ = fs::remove_dir_all(&hold_dir);
        let _ = fs::remove_dir_all(&work);
    }

    #[test]
    fn safe_rel_rejects_traversal() {
        assert!(safe_rel("../etc/passwd").is_err());
        assert!(safe_rel("/etc/passwd").is_err());
        assert!(safe_rel("").is_err());
        assert!(safe_rel("normal/file.php").is_ok());
    }

    #[test]
    fn safe_id_rejects_invalid() {
        assert!(safe_id("").is_err());
        assert!(safe_id("../evil").is_err());
        assert!(safe_id("hold_123_456").is_ok());
    }
}
