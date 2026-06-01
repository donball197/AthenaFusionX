#!/bin/bash
set -e

# 1. Base Crates First (No dependencies)
echo ">> Ignite: Building Base Crates..."
cargo build -p nexus_identity --release
cargo build -p nexus_protocol --release
cargo build -p nexus_mesh --release

# 2. Memory & Substrate (Lower dependency level)
echo ">> Ignite: Building Memory & Substrate..."
cargo build -p nexus_memory_graph --release
cargo build -p nexus_substrate --release

# 3. Core Runtime & Local Reflex
echo ">> Ignite: Building Core Runtime & Reflex..."
cargo build -p nexus_runtime_core --release
cargo build -p reflex_test --release

# 4. Final Binary Linkage
echo ">> Ignite: Linking Binaries..."
cargo build -p nexus_ide_backend --release
cargo build -p nexus_cli --release
cargo build -p node --release
