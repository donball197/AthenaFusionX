Findings:
1. CONFIRMED: `normalize_path` lexically normalizes slash paths, resolves `.` and `..`, collapses duplicate slashes, preserves a leading `/`, and expands only exact `~` or `~/...` to `$HOME`.
2. CONFIRMED: `paths_equivalent` only bridges an absolute path against its workspace-relative suffix; arbitrary suffix overlap does not match.
3. CONFIRMED: `is_confirmed_absence` accepts only missing-target errors and rejects sandbox denials, permission errors, and unrecognized failures.

Scope note: MemoryPalace, CorticalLoop, verifier.rs, and most crates were not traced.
