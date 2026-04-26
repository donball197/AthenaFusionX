#!/usr/bin/env bash
# =============================================================================
# NEXUS SINGULARITY — Upgraded Shell Scripts (cat EOF heredoc style)
# Platform: ARM64 Chromebook / Linux (Penguin)
# Project:  ~/nexus_singularity-dev
# =============================================================================
set -euo pipefail


# =============================================================================
# 1. WASM REGISTRY — Basic Queries
# =============================================================================

cat << 'EOF'
══════════════════════════════════════════════════════
 [1] WASM REGISTRY — List All Cell Names
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
.mode column
.headers on
SELECT name FROM wasm_cells ORDER BY created_at DESC;
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [2] WASM REGISTRY — Full Metadata (no blob)
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
.mode column
.headers on
SELECT name, hit_count, created_at
FROM wasm_cells
ORDER BY created_at DESC;
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [3] WASM REGISTRY — Inspect Cell Source Code
 Usage: set CELL_NAME before running this block
══════════════════════════════════════════════════════
EOF
CELL_NAME="${CELL_NAME:-compute_resonance}"
sqlite3 .nexus_wasm_registry.db << SQLEOF
.mode column
.headers on
SELECT name, source_code, created_at, hit_count
FROM wasm_cells
WHERE name = '${CELL_NAME}';
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [4] WASM REGISTRY — Row Count
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
SELECT COUNT(*) AS total_cells FROM wasm_cells;
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [5] WASM REGISTRY — Top Hit Cells (Most Executed)
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
.mode column
.headers on
SELECT name, hit_count, created_at
FROM wasm_cells
ORDER BY hit_count DESC
LIMIT 10;
SQLEOF


# =============================================================================
# 2. MEMORY PALACE — Neural Context Queries
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [6] MEMORY PALACE — Last 10 Neural Context Entries
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_memory.db << 'SQLEOF'
.mode column
.headers on
SELECT id, origin, directive, timestamp
FROM neural_context
ORDER BY id DESC
LIMIT 10;
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [7] MEMORY PALACE — Search by Origin
 Usage: set ORIGIN before running (e.g. ORIGIN=QUANTUM_INGESTOR)
══════════════════════════════════════════════════════
EOF
ORIGIN="${ORIGIN:-ALPHA-SCOUT}"
sqlite3 .nexus_memory.db << SQLEOF
.mode column
.headers on
SELECT id, origin, directive, result, timestamp
FROM neural_context
WHERE origin = '${ORIGIN}'
ORDER BY id DESC
LIMIT 10;
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [8] MEMORY PALACE — System Insights (last 5)
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_memory.db << 'SQLEOF'
.mode column
.headers on
SELECT id, summary, timestamp
FROM system_insights
ORDER BY id DESC
LIMIT 5;
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [9] MEMORY PALACE — Row Counts All Tables
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_memory.db << 'SQLEOF'
SELECT 'neural_context'  AS tbl, COUNT(*) AS rows FROM neural_context
UNION ALL
SELECT 'system_insights' AS tbl, COUNT(*) AS rows FROM system_insights;
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [10] MEMORY PALACE — Entries WITH Embeddings
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_memory.db << 'SQLEOF'
.mode column
.headers on
SELECT id, origin, directive, timestamp
FROM neural_context
WHERE embedding IS NOT NULL
ORDER BY id DESC
LIMIT 10;
SQLEOF


# =============================================================================
# 3. MISSION QUEUE
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [11] MISSION QUEUE — All Missions
══════════════════════════════════════════════════════
EOF
if [ -f ".nexus_missions.db" ]; then
  sqlite3 .nexus_missions.db << 'SQLEOF'
.mode column
.headers on
SELECT id, goal, status, created_at
FROM missions
ORDER BY created_at DESC
LIMIT 20;
SQLEOF
else
  echo " (no mission queue DB found at .nexus_missions.db)"
fi


cat << 'EOF'

══════════════════════════════════════════════════════
 [12] MISSION QUEUE — Pending Only
══════════════════════════════════════════════════════
EOF
if [ -f ".nexus_missions.db" ]; then
  sqlite3 .nexus_missions.db << 'SQLEOF'
.mode column
.headers on
SELECT id, goal, created_at
FROM missions
WHERE status = 'pending'
ORDER BY created_at ASC;
SQLEOF
fi


# =============================================================================
# 4. SCHEMA INSPECTION — All DBs
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [13] SCHEMA — wasm_registry
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
.schema
SQLEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [14] SCHEMA — memory palace
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_memory.db << 'SQLEOF'
.schema
SQLEOF


# =============================================================================
# 5. LAZARUS VAULT — Snapshot Ledger
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [15] LAZARUS VAULT — List All Snapshots
══════════════════════════════════════════════════════
EOF
LEDGER=".nexus/vault/ledger.json"
if [ -f "$LEDGER" ]; then
  cat "$LEDGER"
else
  cat << 'WARN'
  [WARNING] No ledger.json found at .nexus/vault/ledger.json
  This is a known bug — auto_snapshot() does not write to the ledger.
  Apply the lazarus_vault fix from the review before relying on rollback.
WARN
fi


cat << 'EOF'

══════════════════════════════════════════════════════
 [16] LAZARUS VAULT — List Raw Snapshot Files
══════════════════════════════════════════════════════
EOF
if [ -d ".nexus/vault" ]; then
  ls -lht .nexus/vault/ | head -20
else
  echo " (vault directory does not exist yet)"
fi


# =============================================================================
# 6. WASM REGISTRY — Maintenance Scripts
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [17] WASM REGISTRY — Delete a Named Cell
 Usage: CELL_NAME=signal_gain bash nexus_scripts.sh
══════════════════════════════════════════════════════
EOF
# Only runs if DELETE_CELL env var is explicitly set to avoid accidents
if [ -n "${DELETE_CELL:-}" ]; then
  sqlite3 .nexus_wasm_registry.db << SQLEOF
DELETE FROM wasm_cells WHERE name = '${DELETE_CELL}';
SELECT changes() AS rows_deleted;
SQLEOF
  echo "[OK] Cell '${DELETE_CELL}' removed from registry."
else
  echo " (set DELETE_CELL=<name> to activate this block)"
fi


cat << 'EOF'

══════════════════════════════════════════════════════
 [18] WASM REGISTRY — Reset Hit Counts
══════════════════════════════════════════════════════
EOF
# Only runs if RESET_HITS=1
if [ "${RESET_HITS:-0}" = "1" ]; then
  sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
UPDATE wasm_cells SET hit_count = 0;
SELECT changes() AS rows_updated;
SQLEOF
  echo "[OK] All hit counts reset."
else
  echo " (set RESET_HITS=1 to activate this block)"
fi


# =============================================================================
# 7. MEMORY PALACE — Maintenance Scripts
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [19] MEMORY PALACE — Purge All Neural Context
 WARNING: This wipes all agent memory permanently.
══════════════════════════════════════════════════════
EOF
if [ "${PURGE_MEMORY:-0}" = "1" ]; then
  sqlite3 .nexus_memory.db << 'SQLEOF'
DELETE FROM neural_context;
DELETE FROM memory_search;
DELETE FROM system_insights;
SELECT 'Purge complete.' AS status;
SQLEOF
else
  echo " (set PURGE_MEMORY=1 to activate — this is irreversible)"
fi


cat << 'EOF'

══════════════════════════════════════════════════════
 [20] MEMORY PALACE — Add Missing FTS5 Delete Trigger
 Fixes the stale-index bug identified in the review.
══════════════════════════════════════════════════════
EOF
sqlite3 .nexus_memory.db << 'SQLEOF'
CREATE TRIGGER IF NOT EXISTS neural_context_ad
AFTER DELETE ON neural_context BEGIN
    INSERT INTO memory_search(memory_search, rowid, directive, result)
    VALUES('delete', old.id, old.directive, old.result);
END;

CREATE TRIGGER IF NOT EXISTS neural_context_au
AFTER UPDATE ON neural_context BEGIN
    INSERT INTO memory_search(memory_search, rowid, directive, result)
    VALUES('delete', old.id, old.directive, old.result);
    INSERT INTO memory_search(rowid, directive, result)
    VALUES(new.id, new.directive, new.result);
END;

SELECT 'FTS5 triggers installed.' AS status;
SQLEOF


# =============================================================================
# 8. FULL HEALTH CHECK — Single-pass across all DBs
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [21] NEXUS HEALTH CHECK — All Substrates
══════════════════════════════════════════════════════
EOF

echo "-- WASM Registry --"
sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
SELECT 'wasm_cells' AS table_name, COUNT(*) AS rows FROM wasm_cells;
SQLEOF

echo "-- Memory Palace --"
sqlite3 .nexus_memory.db << 'SQLEOF'
SELECT 'neural_context'  AS table_name, COUNT(*) AS rows FROM neural_context
UNION ALL
SELECT 'system_insights' AS table_name, COUNT(*) AS rows FROM system_insights;
SQLEOF

echo "-- Lazarus Vault --"
if [ -f ".nexus/vault/ledger.json" ]; then
  python3 -c "
import json, sys
with open('.nexus/vault/ledger.json') as f:
    data = json.load(f)
print(f'  snapshots in ledger: {len(data)}')
if data:
    print(f'  latest: {data[-1][\"hash\"]} @ {data[-1][\"timestamp\"]}')
" 2>/dev/null || echo "  (ledger exists but may be empty or malformed)"
else
  echo "  [WARNING] ledger.json missing — rollback is broken (see review fix)"
fi

echo "-- Mission Queue --"
if [ -f ".nexus_missions.db" ]; then
  sqlite3 .nexus_missions.db << 'SQLEOF'
SELECT status, COUNT(*) AS count FROM missions GROUP BY status;
SQLEOF
else
  echo "  (no mission queue DB)"
fi

cat << 'EOF'

══════════════════════════════════════════════════════
 Health check complete.
══════════════════════════════════════════════════════
EOF

