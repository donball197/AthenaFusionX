//! Shared mutation revert. Tracked files are restored via `git checkout`;
//! untracked files (created by the agent) are deleted. Paths that escape
//! the repo root, or point into git metadata, are refused.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir);
    cmd
}

fn repo_root(dir: &Path) -> Result<PathBuf, String> {
    let out =
        git(dir).args(["rev-parse", "--show-toplevel"]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("not inside a git work tree".to_string());
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    PathBuf::from(s).canonicalize().map_err(|e| e.to_string())
}

fn governing_repo(outer_root: &Path, target: &Path) -> PathBuf {
    let mut cur = target.parent();
    while let Some(dir) = cur {
        if !dir.starts_with(outer_root) {
            break;
        }
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        if dir == outer_root {
            break;
        }
        cur = dir.parent();
    }
    outer_root.to_path_buf()
}

/// Ok(true) = tracked, Ok(false) = git says unknown path, Err = git failed.
fn is_tracked(dir: &Path, spec: &str) -> Result<bool, String> {
    let out = git(dir)
        .args(["ls-files", "--error-unmatch", "--", spec])
        .output()
        .map_err(|e| e.to_string())?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        other => Err(format!("git ls-files failed: {:?}", other)),
    }
}

/// Returns Ok(()) only when `path` is back to its pre-mutation state.
pub(crate) fn revert_mutation_path(path: &str) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    revert_in(&cwd, path, &|dir, spec| {
        let dir_s = dir.to_string_lossy().to_string();
        crate::podman::PodmanShield::execute_argv("git", &["-C", &dir_s, "checkout", "--", spec])
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// `dir` is where relative `path`s resolve and where git runs.
/// `checkout` restores one tracked pathspec and reports success.
fn revert_in(dir: &Path, path: &str, checkout: &dyn Fn(&Path, &str) -> bool) -> Result<(), String> {
    let root = repo_root(dir)?;
    let dir_canon = dir.canonicalize().map_err(|e| e.to_string())?;
    let joined = dir_canon.join(path);

    let file_name =
        joined.file_name().ok_or_else(|| "path has no file name".to_string())?.to_os_string();
    let parent = joined.parent().ok_or_else(|| "path has no parent".to_string())?;
    // Parent missing means the file cannot exist: nothing to revert.
    let parent = match parent.canonicalize() {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };
    if !parent.starts_with(&root) {
        return Err(format!("path escapes repo root: {}", path));
    }
    let target = parent.join(file_name);
    let rel = target.strip_prefix(&root).map_err(|e| e.to_string())?.to_string_lossy().to_string();
    if rel.split('/').any(|c| c == ".git") {
        return Err(format!("refusing to touch git metadata: {}", rel));
    }
    // Select the repo that actually tracks `target`. Normally the repo
    // rooted at `root`; for a path inside a nested worktree (e.g.
    // vendor/upstream/x.rs) it is the nested repo.
    let governing = governing_repo(&root, &target);
    let spec =
        target.strip_prefix(&governing).map_err(|e| e.to_string())?.to_string_lossy().to_string();

    if is_tracked(&governing, &spec)? {
        if checkout(&governing, &spec) {
            Ok(())
        } else {
            Err(format!("git checkout failed for {} (repo {})", spec, governing.display()))
        }
    } else if std::fs::symlink_metadata(&target).is_ok() {
        std::fs::remove_file(&target).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Sandbox {
        base: PathBuf,
        repo: PathBuf,
    }

    impl Sandbox {
        fn new(name: &str, init_git: bool) -> Self {
            let base =
                std::env::temp_dir().join(format!("nexus_revert_{}_{}", std::process::id(), name));
            let _ = fs::remove_dir_all(&base);
            let repo = base.join("repo");
            fs::create_dir_all(&repo).unwrap();
            if init_git {
                run(&repo, &["init", "-q"]);
                run(&repo, &["config", "user.email", "t@example.com"]);
                run(&repo, &["config", "user.name", "t"]);
                run(&repo, &["config", "commit.gpgsign", "false"]);
            }
            Sandbox { base, repo }
        }

        fn commit_file(&self, name: &str, content: &str) {
            fs::write(self.repo.join(name), content).unwrap();
            run(&self.repo, &["add", name]);
            run(&self.repo, &["commit", "-q", "-m", "init"]);
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git must be installed");
        assert!(status.success(), "git {:?} failed", args);
    }

    fn checkout_in(_bound: &Path) -> impl Fn(&Path, &str) -> bool {
        |dir, spec| {
            Command::new("git")
                .current_dir(dir)
                .args(["checkout", "--", spec])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
    }

    #[test]
    fn tracked_file_is_restored() {
        let sb = Sandbox::new("tracked", true);
        sb.commit_file("a.txt", "original\n");
        fs::write(sb.repo.join("a.txt"), "mutated\n").unwrap();
        revert_in(&sb.repo, "a.txt", &checkout_in(&sb.repo)).unwrap();
        assert_eq!(fs::read_to_string(sb.repo.join("a.txt")).unwrap(), "original\n");
    }

    #[test]
    fn untracked_file_is_deleted() {
        let sb = Sandbox::new("untracked", true);
        sb.commit_file("a.txt", "x\n");
        fs::write(sb.repo.join("new.rs"), "fn main() {}\n").unwrap();
        revert_in(&sb.repo, "new.rs", &checkout_in(&sb.repo)).unwrap();
        assert!(!sb.repo.join("new.rs").exists());
        assert!(sb.repo.join("a.txt").exists());
    }

    #[test]
    fn missing_file_in_existing_dir_is_ok() {
        let sb = Sandbox::new("missing", true);
        sb.commit_file("a.txt", "x\n");
        revert_in(&sb.repo, "ghost.rs", &checkout_in(&sb.repo)).unwrap();
    }

    #[test]
    fn missing_parent_dir_is_ok() {
        let sb = Sandbox::new("noparent", true);
        sb.commit_file("a.txt", "x\n");
        revert_in(&sb.repo, "no/such/dir/x.rs", &checkout_in(&sb.repo)).unwrap();
    }

    #[test]
    fn relative_escape_is_refused_and_file_untouched() {
        let sb = Sandbox::new("escape", true);
        sb.commit_file("a.txt", "x\n");
        let outside = sb.base.join("outside.txt");
        fs::write(&outside, "keep").unwrap();
        let err = revert_in(&sb.repo, "../outside.txt", &|_, _| true).unwrap_err();
        assert!(err.contains("escapes"), "{}", err);
        assert!(outside.exists());
    }

    #[test]
    fn absolute_path_outside_repo_is_refused() {
        let sb = Sandbox::new("abs", true);
        sb.commit_file("a.txt", "x\n");
        let outside = sb.base.join("outside2.txt");
        fs::write(&outside, "keep").unwrap();
        let abs = outside.to_str().unwrap().to_string();
        let err = revert_in(&sb.repo, &abs, &|_, _| true).unwrap_err();
        assert!(err.contains("escapes"), "{}", err);
        assert!(outside.exists());
    }

    #[test]
    fn not_a_git_repo_never_deletes() {
        let sb = Sandbox::new("nogit", false);
        fs::write(sb.repo.join("f.txt"), "keep").unwrap();
        assert!(revert_in(&sb.repo, "f.txt", &|_, _| true).is_err());
        assert!(sb.repo.join("f.txt").exists());
    }

    #[test]
    fn git_metadata_is_protected() {
        let sb = Sandbox::new("gitdir", true);
        sb.commit_file("a.txt", "x\n");
        let evil = sb.repo.join(".git").join("evil.txt");
        fs::write(&evil, "keep").unwrap();
        let err = revert_in(&sb.repo, ".git/evil.txt", &|_, _| true).unwrap_err();
        assert!(err.contains(".git"), "{}", err);
        assert!(evil.exists());
    }

    #[test]
    fn works_from_a_subdirectory() {
        let sb = Sandbox::new("subdir", true);
        sb.commit_file("a.txt", "original\n");
        let sub = sb.repo.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sb.repo.join("a.txt"), "mutated\n").unwrap();
        revert_in(&sub, "../a.txt", &checkout_in(&sub)).unwrap();
        assert_eq!(fs::read_to_string(sb.repo.join("a.txt")).unwrap(), "original\n");
    }

    #[cfg(unix)]
    #[test]
    fn dangling_untracked_symlink_is_removed() {
        let sb = Sandbox::new("symlink", true);
        sb.commit_file("a.txt", "x\n");
        let link = sb.repo.join("link.rs");
        std::os::unix::fs::symlink("/nonexistent/target", &link).unwrap();
        revert_in(&sb.repo, "link.rs", &checkout_in(&sb.repo)).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());
    }
    #[test]
    fn nested_repo_file_is_restored_not_deleted() {
        // Regression: a path tracked by a nested repo must be restored
        // from that repo, not deleted as if it were an agent artifact.
        let outer = Sandbox::new("nested", true);
        let inner = outer.repo.join("inner");
        fs::create_dir_all(&inner).unwrap();
        run(&inner, &["init", "-q"]);
        run(&inner, &["config", "user.email", "t@example.com"]);
        run(&inner, &["config", "user.name", "t"]);
        fs::write(inner.join("x.rs"), "orig").unwrap();
        run(&inner, &["add", "."]);
        run(&inner, &["commit", "-qm", "init"]);

        fs::write(inner.join("x.rs"), "mutated").unwrap();

        revert_in(&outer.repo, "inner/x.rs", &checkout_in(&outer.repo)).unwrap();

        assert_eq!(
            fs::read_to_string(inner.join("x.rs")).unwrap(),
            "orig",
            "nested-repo file must be restored, not deleted"
        );
        assert!(inner.join("x.rs").exists());
    }
}
