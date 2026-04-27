#!/bin/bash
echo ">> [NEXUS] Verifying quantum_architecture.md and kernel..."
head -40 src/quantum_architecture.md
echo -e "\n---\n"
cargo check --quiet && echo "✅ cargo check PASSED" || echo "❌ cargo check FAILED"
ls -l src/quantum_architecture.md
echo ">> Ready. Paste the directive again in the factory."
