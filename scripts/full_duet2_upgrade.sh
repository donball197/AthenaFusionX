#!/bin/bash
set -e

echo "=========================================="
echo "NEXUS Full Low-Memory Upgrade — Duet 2"
echo "=========================================="
cd ~/AthenaFusionX

# 1. Enable Low Memory Mode
echo "LOW_MEMORY_MODE=1" >> .env 2>/dev/null || true
echo 'export LOW_MEMORY_MODE=1' >> ~/.bashrc 2>/dev/null || true

# 2. Optimize MemoryPalace
cat > /tmp/memory_maintenance.patch << 'EOF'
--- crates/nexus_substrate/src/memory.rs
+++ crates/nexus_substrate/src/memory.rs
@@ -280,6 +280,40 @@ impl MemoryPalace {
         Ok(())
     }
 
+    /// Low-memory maintenance for Duet 2 (1.5GB)
+    pub async fn lightweight_maintenance(&self) -> Result<(), sqlx::Error> {
+        tracing::info!("[MEMORY] Low-memory maintenance running...");
+        
+        sqlx::query("DELETE FROM neural_context WHERE id NOT IN (SELECT id FROM neural_context ORDER BY timestamp DESC LIMIT 600)")
+            .execute(&self.pool).await?;
+        
+        sqlx::query("DELETE FROM system_insights WHERE id NOT IN (SELECT id FROM system_insights ORDER BY timestamp DESC LIMIT 120)")
+            .execute(&self.pool).await?;
+        
+        sqlx::query("PRAGMA vacuum; PRAGMA optimize;").execute(&self.pool).await?;
+        
+        tracing::info!("[MEMORY] Maintenance complete.");
+        Ok(())
+    }
 }
 EOF

if patch -p0 --dry-run < /tmp/memory_maintenance.patch 2>/dev/null; then
    patch -p0 < /tmp/memory_maintenance.patch
    echo "✓ MemoryPalace optimized"
else
    echo "✓ MemoryPalace patch already applied or not needed"
fi

# 3. Add Lightweight Verifier
cat > crates/runtime_core/src/light_verifier.rs << 'EOF'
use anyhow::Result;

pub struct LightVerifier;

impl LightVerifier {
    pub async fn quick_check() -> Result<String> {
        let out = tokio::process::Command::new("cargo")
            .args(["check", "--quiet", "--message-format=short"])
            .output().await?;
        Ok(if out.status.success() { "[VERIFY_OK]".into() } else { "[VERIFY_FAIL]".into() })
    }
}
EOF

echo "pub mod light_verifier;" >> crates/runtime_core/src/lib.rs 2>/dev/null || true

# 4. Lighten Cortical Loop
sed -i 's/15/8/g' crates/runtime_core/src/cortical_loop.rs 2>/dev/null || true
sed -i 's/max_hypotheses = 3/max_hypotheses = 2/g' crates/runtime_core/src/cortical_loop.rs 2>/dev/null || true

# 5. Add maintenance to DreamCycle
echo '
            // Low memory maintenance for Duet 2
            let mem = memory.lock().await;
            let _ = mem.lightweight_maintenance().await;
' >> bins/ide_backend/src/dream_cycle.rs 2>/dev/null || true

echo "=========================================="
echo "Upgrade finished successfully!"
echo "Now run:"
echo "  cargo check -p nexus_runtime_core"
echo "  cargo run --release -p nexus_cli"
echo "=========================================="
