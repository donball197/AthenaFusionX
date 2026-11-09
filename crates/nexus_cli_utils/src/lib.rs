//! Shared CLI helpers for `apps/*` binaries.
//! Consolidated from the 2026-11-09 workspace audit.

use anyhow::Context;
use std::path::Path;

/// Scan `args` for any of `flags`, returning the following argument.
/// Matches the first flag in `flags` order. Refuses to treat a
/// `-`-prefixed token as a flag's value (matches the four pre-audit
/// copies: symbol_graph, mutation_ranker, execution_auditor,
/// benchmark_runner).
pub fn parse_arg(args: &[String], flags: &[&str]) -> Option<String> {
    for flag in flags {
        if let Some(i) = args.iter().position(|r| r == flag) {
            if let Some(val) = args.get(i + 1) {
                if !val.starts_with('-') {
                    return Some(val.clone());
                }
            }
        }
    }
    None
}

/// Ensure the parent directory of `path` exists. Bare filenames
/// (`Path::new("out.json")`) have `parent() == Some("")`, which is
/// handled explicitly as a no-op.
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Write `content` to `path`, ensuring the parent directory exists.
/// Directory-creation failures are attributed to directory creation,
/// not to the subsequent `fs::write` (which would otherwise see ENOENT
/// and report "No such file or directory" — the pre-audit bug).
pub fn write_text_output(path: &Path, content: &str) -> anyhow::Result<()> {
    ensure_parent_dir(path)
        .with_context(|| format!("Failed to create directory for {:?}", path))?;
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write output to {:?}", path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_arg_finds_flag() {
        let args = vec!["--in".into(), "f.rs".into()];
        assert_eq!(parse_arg(&args, &["--in"]), Some("f.rs".into()));
    }

    #[test]
    fn parse_arg_accepts_any_of_flags() {
        let args = vec!["-i".into(), "f.rs".into()];
        assert_eq!(parse_arg(&args, &["--in", "-i"]), Some("f.rs".into()));
    }

    #[test]
    fn parse_arg_none_when_absent() {
        let args = vec!["--other".into(), "x".into()];
        assert_eq!(parse_arg(&args, &["--in"]), None);
    }

    #[test]
    fn parse_arg_none_when_flag_is_last() {
        let args = vec!["--in".into()];
        assert_eq!(parse_arg(&args, &["--in"]), None);
    }

    #[test]
    fn parse_arg_skips_trailing_flag() {
        let args = vec!["--in".into(), "--output".into(), "f.json".into()];
        assert_eq!(parse_arg(&args, &["--in"]), None);
    }

    #[test]
    fn parse_arg_finds_value_after_sibling_flag() {
        let args = vec!["--in".into(), "--output".into(), "f.json".into()];
        assert_eq!(parse_arg(&args, &["--output"]), Some("f.json".into()));
    }

    #[test]
    fn parse_arg_priority_follows_flag_order() {
        let args = vec!["-i".into(), "a".into(), "--in".into(), "b".into()];
        assert_eq!(parse_arg(&args, &["--in", "-i"]), Some("b".into()));
        assert_eq!(parse_arg(&args, &["-i", "--in"]), Some("a".into()));
    }

    #[test]
    fn write_text_output_creates_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a/b/c/out.json");
        write_text_output(&path, "{}").unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    }

    #[test]
    fn write_text_output_bare_filename() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = write_text_output(Path::new("out.json"), "{}");
        std::env::set_current_dir(cwd).unwrap();
        result.unwrap();
        assert!(tmp.path().join("out.json").exists());
    }
}
