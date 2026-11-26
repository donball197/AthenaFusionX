// forever_fix_dryrun.rs — side-by-side current vs permanent classification
// Run: cargo test -p nexus_runtime_core forever_fix_dryrun -- --nocapture

#[cfg(test)]
mod forever_fix_dryrun {
    fn current_is_mutation(criteria: &str, objective: &str) -> (bool, bool) {
        let declared_mutation = criteria.contains("WRITE_OK:")
            || criteria.contains("PATCH_FUNCTION_OK:")
            || criteria.contains("DIFF_PATCH_OK:")
            || criteria.contains("REPLACE_LINES_OK:");
        let read_only_intent = objective.to_lowercase().contains("read-only")
            || objective.to_lowercase().contains("read only")
            || objective.to_lowercase().contains("do not modify");
        let read_only = !declared_mutation && read_only_intent;
        let has_write = objective.to_lowercase().contains("write")
            || criteria.contains("WRITE_OK:");
        let is_mutation = !read_only && (has_write
            || criteria.contains("PATCH_FUNCTION_OK:")
            || criteria.contains("DIFF_PATCH_OK:")
            || criteria.contains("REPLACE_LINES_OK:"));
        (read_only, is_mutation)
    }

    fn forever_is_mutation(criteria: &str, objective: &str) -> (bool, bool) {
        let declared_source_mutation = criteria.contains("PATCH_FUNCTION_OK:")
            || criteria.contains("DIFF_PATCH_OK:")
            || criteria.contains("REPLACE_LINES_OK:")
            || criteria.lines().any(|l| {
                let t = l.trim_start();
                t.starts_with("WRITE_OK:")
                    && t.strip_prefix("WRITE_OK:")
                        .map(|p| p.trim().ends_with(".rs"))
                        .unwrap_or(false)
            });
        let read_only_intent = objective.to_lowercase().contains("read-only")
            || objective.to_lowercase().contains("read only")
            || objective.to_lowercase().contains("do not modify");
        let read_only = !declared_source_mutation && read_only_intent;

        let write_targets: Vec<&str> = criteria
            .lines()
            .filter_map(|l| {
                let t = l.trim_start();
                t.strip_prefix("WRITE_OK:").map(|p| p.trim())
            })
            .collect();
        let any_source_write = write_targets.iter().any(|p| p.ends_with(".rs"));
        let has_source_patch = criteria.contains("PATCH_FUNCTION_OK:")
            || criteria.contains("DIFF_PATCH_OK:")
            || criteria.contains("REPLACE_LINES_OK:");

        let is_mutation = !read_only && (any_source_write || has_source_patch);
        (read_only, is_mutation)
    }

    fn row(name: &str, criteria: &str, objective: &str) {
        let (c_ro, c_mut) = current_is_mutation(criteria, objective);
        let (f_ro, f_mut) = forever_is_mutation(criteria, objective);
        let changed = (c_ro != f_ro) || (c_mut != f_mut);
        println!("┌─ {}", name);
        println!("│  current : read_only={}  is_mutation={}", c_ro, c_mut);
        println!("│  forever : read_only={}  is_mutation={}", f_ro, f_mut);
        if changed {
            println!("│  *** BEHAVIOR CHANGE (this is the fix) ***");
        }
        println!("└──────────────────────────────────────────────");
    }

    #[test]
    fn forever_classification_table() {
        println!("\n=== Forever fix — classification dry-run ===\n");

        row(
            "Mission 1 (read-only audit + report .md)",
            "WRITE_OK:mission_01_completion_audit.md\nREAD_OK:crates/runtime_core/src",
            "Read-only investigation. Do not modify source. Write the result to mission_01_completion_audit.md",
        );

        row(
            "Real source patch mission",
            "PATCH_FUNCTION_OK:crates/runtime_core/src/foo.rs:bar\nREAD_OK:crates/runtime_core/src/foo.rs",
            "Patch function bar in foo.rs",
        );

        row(
            "WRITE_OK of .rs source",
            "WRITE_OK:crates/runtime_core/src/foo.rs",
            "Write the fixed implementation to foo.rs",
        );

        row(
            "Pure read-only recon",
            "READ_OK:crates/runtime_core/src\nSEARCH_CHECKED_OK:crates/runtime_core/src:worker",
            "READ-ONLY. Inspect worker recovery paths. Do not modify any files.",
        );

        row(
            "Report write, no read-only keyword",
            "WRITE_OK:report.md\nREAD_OK:crates/runtime_core/src",
            "Investigate completion logic and write findings to report.md",
        );

        println!("\nExpected:");
        println!("  Mission 1          → forever: read_only=true  is_mutation=false");
        println!("  Real source patch  → forever: read_only=false is_mutation=true");
        println!("  WRITE_OK .rs       → forever: read_only=false is_mutation=true");
        println!("  Pure read-only     → forever: read_only=true  is_mutation=false");
        println!("=== dry-run complete ===\n");
    }
}
