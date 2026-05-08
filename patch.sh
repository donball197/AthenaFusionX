#!/bin/bash

echo -e "\x1b[35m>> Configuring Cargo for ARM64 Zero Fragmentation...\x1b[0m"
mkdir -p .cargo

# 1. Write the optimized Cargo Config
cat << 'EOF' > .cargo/config.toml
[build]
jobs = 1

# Target-specific linker to protect the Termux/Android build process
[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[env]
CARGO_INCREMENTAL = "1"
RUST_BACKTRACE = "1"
EOF

echo -e "\x1b[35m>> Writing optimized workspace Cargo.toml...\x1b[0m"

# 2. Write the optimized Workspace Cargo.toml
cat << 'EOF' > Cargo.toml
[workspace]
members = [
    "crates/runtime_core",
    "crates/identity",
    "crates/memory_graph",
    "crates/economy",
    "crates/protocol",
    "crates/mesh",
    "crates/payments",
    "crates/marketplace",
    "crates/billing",
    "bins/cli",
    "bins/node",
    "bins/ide_backend"
]
resolver = "2"

[workspace.package]
edition = "2021"
version = "1.9.0"

[workspace.dependencies]
tokio       = { version = "1.44", features = ["full"] }
serde       = { version = "1.0",  features = ["derive"] }
serde_json  = "1.0"
anyhow      = "1.0"
reqwest     = { version = "0.12", features = ["json", "stream"] }
axum        = { version = "0.7",  features = ["ws", "macros"] }
sqlx        = { version = "0.8",  features = ["sqlite", "runtime-tokio", "tls-rustls"] }
tracing     = "0.1"
once_cell   = "1.19"
parking_lot = "0.12"
async-trait = "0.1"
base64      = "0.22"
uuid        = { version = "1.8",  features = ["v4", "serde"] }

[profile.dev]
opt-level       = 0
debug           = true
incremental     = true
codegen-units   = 16
panic           = "abort"

[profile.release]
opt-level     = 3
lto           = "thin"
codegen-units = 1
strip         = "symbols"
EOF

echo -e "\x1b[35m>> Executing aggressive clean...\x1b[0m"

# 3. Standard protocol: Wipe target artifacts and orphaned shared objects
rm -rf target/
rm -f *.so

echo -e "\x1b[32m>> Setup complete. Ready for compilation.\x1b[0m"
