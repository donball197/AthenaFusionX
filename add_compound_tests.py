#!/usr/bin/env python3
"""
add_compound_tests.py

Appends two acceptance tests to the epistemic_engine test module:

  compound_read_plus_memory_pushes_search_memory_first
  compound_read_plus_search_code_pushes_search_first

These prove the compound READ_OK: block now orders search before read
and that SEARCH_MEMORY_OK is no longer dropped when READ_OK is present.

Anchor is the full last test + closing brace of the module. Aborts
without writing if the anchor doesn't match verbatim.

Dry-run by default. Pass --apply to write.
"""
import argparse, shutil, sys
from pathlib import Path

TARGET = Path("crates/runtime_core/src/epistemic_engine.rs")

ANCHOR = '''\
        assert_eq!(strategies.len(), 1);
        assert_eq!(strategies[0], format!("search_memory:{}", directive));
    }
}
'''

INSERT = '''\
        assert_eq!(strategies.len(), 1);
        assert_eq!(strategies[0], format!("search_memory:{}", directive));
    }

    #[tokio::test]
    async fn compound_read_plus_memory_pushes_search_memory_first() {
        let directive = r#"OBJECTIVE: Inspect the compiler-critic failure branch.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
READ_OK:crates/runtime_core/src/cortical_loop.rs
SEARCH_MEMORY_OK:compiler_critic_prior_lessons

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced."#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("compound READ + MEMORY mission should produce strategies");

        assert_eq!(
            strategies[0],
            "search_memory:compiler_critic_prior_lessons",
            "search_memory must be selected before read_file; got {:?}",
            strategies
        );
        assert!(
            strategies.iter().any(|s| s == "read_file:crates/runtime_core/src/cortical_loop.rs"),
            "read_file must still be present as a fallback"
        );
    }

    #[tokio::test]
    async fn compound_read_plus_search_code_pushes_search_first() {
        let directive = r#"OBJECTIVE: Inspect the compiler-critic failure branch.
ASSIGNED ROLE: CodeAnalyzer

MANDATORY NODE SUCCESS CRITERIA:
READ_OK:crates/runtime_core/src/cortical_loop.rs
SEARCH_CODE_OK:compiler_critic

NODE COMPLETION RULE: Every mandatory criterion must be physically evidenced."#;

        let strategies = EpistemicEngine::generate_hypotheses(&Client::new(), "", directive, "")
            .await
            .expect("compound READ + SEARCH_CODE mission should produce strategies");

        assert!(
            strategies[0].starts_with("search_code:"),
            "search_code must be selected before read_file; got {:?}",
            strategies
        );
        assert!(
            strategies.iter().any(|s| s == "read_file:crates/runtime_core/src/cortical_loop.rs"),
            "read_file must still be present as a fallback"
        );
    }
}
'''

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--apply", action="store_true")
    args = ap.parse_args()

    if not TARGET.exists():
        print(f"[ABORT] {TARGET} not found")
        return 2

    src = TARGET.read_text(encoding="utf-8")
    count = src.count(ANCHOR)
    if count != 1:
        print(f"[ABORT] anchor matches {count} times (expected 1)")
        return 1
    print(f"[ok] anchor: 1 match")

    patched = src.replace(ANCHOR, INSERT, 1)
    if patched == src:
        print("[ABORT] identical output")
        return 1

    if not args.apply:
        print("Dry-run complete. Re-run with --apply to write.")
        return 0

    bak = TARGET.with_suffix(TARGET.suffix + ".pre-compound-tests.bak")
    if not bak.exists():
        shutil.copy2(TARGET, bak)
        print(f"[bak] {bak}")
    TARGET.write_text(patched, encoding="utf-8")
    print(f"[write] {TARGET} updated")
    return 0

if __name__ == "__main__":
    sys.exit(main())
