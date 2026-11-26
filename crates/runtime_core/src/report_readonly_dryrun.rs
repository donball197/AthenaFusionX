// report_readonly_dryrun.rs
// cargo test -p nexus_runtime_core report_readonly_dryrun -- --nocapture

#[cfg(test)]
mod report_readonly_dryrun {
    /// Approximate pre-fix: read_only clears all artifacts / write conditions.
    fn current_allows_report(read_only_intent: bool, write_path: &str) -> (bool /*ro*/, bool /*mutation*/, bool /*can_write_report*/) {
        let is_rs = write_path.trim().ends_with(".rs");
        let declared_source = is_rs;
        let read_only = !declared_source && read_only_intent;
        // old behavior under read_only: no arts, no WritePathAcquired
        let can_write = !read_only;
        let mutation = !read_only && (is_rs || !write_path.is_empty());
        (read_only, mutation, can_write)
    }

    /// Desired: non-.rs report write allowed under read_only; is_mutation_task stays false.
    fn fixed_allows_report(read_only_intent: bool, write_path: &str) -> (bool, bool, bool) {
        let is_rs = write_path.trim().ends_with(".rs");
        let declared_source = is_rs;
        let read_only = !declared_source && read_only_intent;
        let can_write_report = !is_rs; // report always allowed; source never under ro
        let mutation = !read_only && is_rs;
        let can_write = if read_only { can_write_report } else { true };
        (read_only, mutation, can_write)
    }

    fn row(name: &str, ro_intent: bool, path: &str) {
        let (c_ro, c_mut, c_w) = current_allows_report(ro_intent, path);
        let (f_ro, f_mut, f_w) = fixed_allows_report(ro_intent, path);
        let changed = (c_ro, c_mut, c_w) != (f_ro, f_mut, f_w);
        println!("┌─ {}", name);
        println!("│  current : read_only={}  mutation={}  can_write_report={}", c_ro, c_mut, c_w);
        println!("│  fixed   : read_only={}  mutation={}  can_write_report={}", f_ro, f_mut, f_w);
        if changed {
            println!("│  *** BEHAVIOR CHANGE ***");
        }
        println!("└──────────────────────────────────────────────");
    }

    #[test]
    fn report_under_readonly_table() {
        println!("\n=== Report-under-read-only dry-run ===\n");
        row(
            "Audit + system_audit_report.md",
            true,
            "system_audit_report.md",
        );
        row(
            "Audit + mission_01_completion_audit.md",
            true,
            "mission_01_completion_audit.md",
        );
        row(
            "Read-only + WRITE_OK foo.rs (must stay blocked)",
            true,
            "crates/runtime_core/src/foo.rs",
        );
        row(
            "No RO intent + report.md",
            false,
            "report.md",
        );
        row(
            "No RO intent + foo.rs",
            false,
            "crates/runtime_core/src/foo.rs",
        );
        println!("\nExpected fixed:");
        println!("  Audit + report.md  → read_only=true  mutation=false  can_write=true");
        println!("  Audit + foo.rs     → read_only=true  mutation=false  can_write=false");
        println!("  No RO + report.md  → read_only=false mutation=false can_write=true");
        println!("  No RO + foo.rs     → read_only=false mutation=true  can_write=true");
        println!("=== done ===\n");
    }
}
