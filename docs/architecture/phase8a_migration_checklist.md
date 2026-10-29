# Phase 8A v3 Migration Checklist

## Manual steps

1. Cargo.toml – add feature cognition_v5 = []
2. lib.rs – add #[cfg(feature = "cognition_v5")] pub mod cognition;
3. cargo check -p nexus_runtime_core
4. cargo check -p nexus_runtime_core --features cognition_v5

Architecture version: 5.0.0-alpha
