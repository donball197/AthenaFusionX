#!/bin/bash
# Location: Workspace Root
# Target: Universal src/ (task_manager.rs)

TARGET_FILE="crates/runtime_core/src/task_manager.rs"

echo "[1/4] Executing GREP to isolate trivial VerificationResult::Success bypass..."
# Isolate the exact pattern where the verifier is blindly trusted
grep -n -B 2 -A 2 "VerificationResult::Success" $TARGET_FILE

echo "[2/4] Deploying surgical AST patch to universal src/..."
# We use a localized patch to intercept the verifier output and force a failure 
# if the criteria vector is empty, killing the false positive before it evaluates.
cat << 'EOF' > verifier_strike.patch
--- crates/runtime_core/src/task_manager.rs
+++ crates/runtime_core/src/task_manager.rs
@@ -304,6 +304,10 @@
-    let verification_result = Verifier::verify_contract(
-        &execution_evidence, 
-        verification_criteria
-    );
+    let verification_result = if verification_criteria.is_empty() {
+        VerificationResult::Failed("CRITICAL: No success criteria defined. Failing closed.".to_string())
+    } else {
+        Verifier::verify_contract(&execution_evidence, verification_criteria)
+    };
EOF

patch $TARGET_FILE < verifier_strike.patch
rm verifier_strike.patch

echo "[3/4] Purging ARM64 DNA caches to prevent cross-node fragmentation..."
# Critical step for dual-node development. Never assume .so or target/ portability.
rm -rf target/
rm -f *.so

echo "[4/4] Sourcing environment and rebuilding Project Nexus Singularity v0.2.2..."
# Ensuring dynamic library seating and STRICT Gemma 3 27B-it API routing.
source universal_ignite.sh
cargo build --release

echo "[SYSTEM] DAG Verification leak sealed. Baseline rebuilt."
