#!/usr/bin/env bash
# =============================================================================
# NEXUS SINGULARITY — Upgraded Admin Utility (v1.0)
# =============================================================================
set -euo pipefail

# [1] WASM REGISTRY — List All Cell Names
echo "══════════════════════════════════════════════════════"
echo " [1] WASM REGISTRY — List All Cell Names"
echo "══════════════════════════════════════════════════════"
sqlite3 .nexus_wasm_registry.db << 'SQLEOF'
.mode column
.headers on
SELECT name FROM wasm_cells ORDER BY created_at DESC;
SQLEOF

# [6] MEMORY PALACE — Last  Neural Context Entries
echo -e "\n══════════════════════════════════════════════════════"
echo " [2] MEMORY PALACE — Recent Context"
echo "══════════════════════════════════════════════════════"
sqlite3 .nexus_memory.db << 'SQLEOF'
.mode column
.headers on
SELECT id, origin, directive, timestamp FROM neural_context ORDER BY id DESC LIMIT 5;
SQLEOF

# [20] MEMORY PALACE — Add Missing FTS5 Triggers
echo -e "\n══════════════════════════════════════════════════════"
echo " [3] REPAIRING SUBSTRATE — Installing FTS5 Triggers"
echo "══════════════════════════════════════════════════════"
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
SQLEOF
echo "[OK] FTS5 integrity triggers active."

# [21] NEXUS HEALTH CHECK — All Substrates
echo -e "\n══════════════════════════════════════════════════════"
echo " [4] NEXUS HEALTH CHECK — All Substrates"
echo "══════════════════════════════════════════════════════"

echo "-- WASM Registry --"
sqlite3 .nexus_wasm_registry.db "SELECT 'wasm_cells' AS table_name, COUNT(*) AS rows FROM wasm_cells;"

echo "-- Memory Palace --"
sqlite3 .nexus_memory.db "SELECT 'neural_context' AS table_name, COUNT(*) AS rows FROM neural_context UNION ALL SELECT 'system_insights' AS table_name, COUNT(*) AS rows FROM system_insights;"

echo "-- Lazarus Vault --"
if [ -f ".nexus/vault/ledger.json" ]; then
    python3 -c "import json; d=json.load(open('.nexus/vault/ledger.json')); print(f'  snapshots in ledger: {len(d)}')" 2>/dev/null || echo "  (ledger empty)"
else
    echo "  [WARNING] ledger.json missing"
fi

echo -e "\n══════════════════════════════════════════════════════"
echo " Health check complete."
echo "══════════════════════════════════════════════════════"
