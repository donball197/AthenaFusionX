# 1. Create the Code Auditor Cargo.toml
cat > apps/code_auditor/Cargo.toml << 'EOF'
[package]
name = "code_auditor"
version = "0.1.0"
edition = "2021"

[dependencies]
nexus_runtime_core = { path = "../../crates/runtime_core" }
nexus_model_hub = { path = "../../crates/nexus_model_hub" }
reqwest = { version = "0.11", features = ["json"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
tracing = "0.1"
chrono = "0.4"
EOF

# 2. Inject it into the root Cargo.toml workspace members
if ! grep -q '"apps/code_auditor"' Cargo.toml; then
    echo ">> Injecting code_auditor into root workspace..."
    sed -i '/\[workspace\]/,/members = \[/ { /members = \[/a\    "apps/code_auditor",
}' Cargo.toml
fi

cargo build --release -p code_auditor
