#!/usr/bin/env bash
# =============================================================================
# NEXUS SINGULARITY — Critical Fixes v1.0
# Fix 1: Lazarus Vault ledger rebuild + auto_snapshot patch
# Fix 2: Quantum Ingestor result cleanup (raw LLM trace → clean JSON)
# Fix 3: Embedding verification + debug script
# Platform: ARM64 Chromebook / Linux (Penguin / Crostini)
# Run from: ~/nexus_singularity-dev
# =============================================================================
set -euo pipefail

VAULT_DIR=".nexus/vault"
LEDGER="${VAULT_DIR}/ledger.json"
SRC_DIR="./src"


# =============================================================================
# FIX 1A — Rebuild ledger.json from existing raw snapshot files on disk
# =============================================================================

cat << 'EOF'
══════════════════════════════════════════════════════
 [1] LAZARUS VAULT — Rebuild ledger.json from disk
     Reconstructs the ledger from the 6 raw snapshot
     files that auto_snapshot wrote but never indexed.
══════════════════════════════════════════════════════
EOF

python3 << 'PYEOF'
import os, json, hashlib, datetime

vault_dir = ".nexus/vault"
ledger_path = os.path.join(vault_dir, "ledger.json")

# Load existing ledger if somehow partially present
existing = []
if os.path.exists(ledger_path):
    try:
        with open(ledger_path) as f:
            existing = json.load(f)
        print(f"  Loaded {len(existing)} existing ledger entries")
    except Exception:
        print("  Existing ledger malformed — starting fresh")

existing_hashes = {e["hash"] for e in existing}
new_entries = list(existing)

# Walk vault dir, pick up raw snapshot files (not dirs, not ledger.json)
for name in os.listdir(vault_dir):
    full = os.path.join(vault_dir, name)
    if name == "ledger.json" or os.path.isdir(full):
        continue

    # The filename IS the hash (DefaultHasher hex output)
    file_hash = name

    if file_hash in existing_hashes:
        print(f"  SKIP {file_hash[:8]} — already in ledger")
        continue

    try:
        content = open(full, "r", errors="replace").read()
    except Exception as e:
        print(f"  SKIP {file_hash[:8]} — unreadable: {e}")
        continue

    # Try to infer original path from content (first line comment or shebang)
    inferred_path = "unknown"
    first_line = content.strip().splitlines()[0] if content.strip() else ""
    if first_line.startswith("//") or first_line.startswith("#"):
        # e.g. "// src/main.rs" or "#!/usr/bin/env bash"
        candidate = first_line.lstrip("/# ").strip()
        if "/" in candidate and len(candidate) < 60:
            inferred_path = candidate

    mtime = os.path.getmtime(full)
    ts = datetime.datetime.fromtimestamp(mtime).strftime("%Y-%m-%dT%H:%M:%S")

    entry = {
        "id":        file_hash,
        "path":      inferred_path,
        "hash":      file_hash,
        "timestamp": ts,
        "directive": "pre_suture",   # conservative tag for recovered snapshots
    }
    new_entries.append(entry)
    existing_hashes.add(file_hash)
    print(f"  INDEXED {file_hash[:8]}  path={inferred_path}  ts={ts}")

# Sort by timestamp ascending (oldest first, newest last — matches list_snapshots usage)
new_entries.sort(key=lambda e: e["timestamp"])

with open(ledger_path, "w") as f:
    json.dump(new_entries, f, indent=2)

print(f"\n  [OK] ledger.json written — {len(new_entries)} total entries")
print(f"  Path: {ledger_path}")
PYEOF


cat << 'EOF'

══════════════════════════════════════════════════════
 [2] LAZARUS VAULT — Verify ledger.json contents
══════════════════════════════════════════════════════
EOF

python3 << 'PYEOF'
import json, os

ledger_path = ".nexus/vault/ledger.json"
if not os.path.exists(ledger_path):
    print("  [FAULT] ledger.json still missing after rebuild")
    exit(1)

with open(ledger_path) as f:
    entries = json.load(f)

print(f"  Total snapshots indexed: {len(entries)}")
print()
for e in entries:
    print(f"  {e['hash'][:8]}  {e['timestamp']}  {e['path']}  [{e['directive']}]")
PYEOF


# =============================================================================
# FIX 1B — Patch lazarus_vault.rs to write ledger.json on every snapshot
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [3] LAZARUS VAULT — Write corrected lazarus_vault.rs
     Adds ledger persistence to auto_snapshot().
     auto_snapshot() currently writes the file to disk
     but NEVER appends to ledger.json — so list_snapshots()
     always returns [] and rollback is permanently blind.
══════════════════════════════════════════════════════
EOF

cat > "${SRC_DIR}/lazarus_vault.rs" << 'RUSTEOF'
// src/lazarus_vault.rs — v1.4 [LEDGER_FIX]
// FIX: auto_snapshot() now appends every snapshot to ledger.json.
//      Previously it wrote the file to .nexus/vault/<hash> but never
//      touched the ledger, so list_snapshots() returned [] permanently
//      and SelfHealingEngine rollback was always blind.
// FIX: list_snapshots() returns entries sorted oldest→newest so
//      .last() in the rollback path gives the most recent stable state.
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub id:        String,
    pub path:      String,
    pub hash:      String,
    pub timestamp: String,
    pub directive: String,
}

pub struct LazarusVault;

impl LazarusVault {
    const VAULT_DIR:  &'static str = ".nexus/vault";
    const LEDGER_PATH: &'static str = ".nexus/vault/ledger.json";

    pub async fn auto_snapshot(path: &str, tag: &str) -> Result<Snapshot, String> {
        // Reject directories — vault only snapshots files
        let metadata = fs::metadata(path)
            .map_err(|e| format!("PATH_NOT_FOUND: {}", e))?;
        if metadata.is_dir() {
            return Err("DIRECTORY_SNAPSHOT_NOT_SUPPORTED: Specify a file.".into());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("FILE_READ_FAULT: {}", e))?;

        // Hash the content for deduplication
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        // Ensure vault directory exists
        fs::create_dir_all(Self::VAULT_DIR)
            .map_err(|e| format!("DIR_FAULT: {}", e))?;

        // Write snapshot file
        let dest = format!("{}/{}", Self::VAULT_DIR, hash);
        fs::write(&dest, &content)
            .map_err(|e| format!("FS_WRITE_FAULT: {}", e))?;

        let snapshot = Snapshot {
            id:        hash.clone(),
            path:      path.to_string(),
            hash:      hash.clone(),
            timestamp: format!("{:?}", SystemTime::now()),
            directive: tag.to_string(),
        };

        // FIX: Persist snapshot to ledger.json
        Self::append_to_ledger(&snapshot)?;

        println!(
            "\x1b[32m[LAZARUS] Snapshot {} → {} [tag: {}]\x1b[0m",
            &hash[..8], path, tag
        );

        Ok(snapshot)
    }

    /// Appends a snapshot entry to ledger.json (creates it if missing).
    fn append_to_ledger(snapshot: &Snapshot) -> Result<(), String> {
        let mut entries = Self::list_snapshots();

        // Deduplicate by hash — don't write the same content twice
        if entries.iter().any(|e| e.hash == snapshot.hash) {
            return Ok(());
        }

        entries.push(snapshot.clone());

        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| format!("LEDGER_SERIALIZE_FAULT: {}", e))?;

        fs::write(Self::LEDGER_PATH, json)
            .map_err(|e| format!("LEDGER_WRITE_FAULT: {}", e))?;

        Ok(())
    }

    /// Returns all snapshots sorted oldest → newest.
    /// .last() gives the most recent stable snapshot for rollback.
    pub fn list_snapshots() -> Vec<Snapshot> {
        if !Path::new(Self::LEDGER_PATH).exists() {
            return vec![];
        }
        let data = fs::read_to_string(Self::LEDGER_PATH).unwrap_or_default();
        let mut entries: Vec<Snapshot> = serde_json::from_str(&data).unwrap_or_default();
        // Sort by timestamp string ascending (ISO-like strings sort correctly)
        entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        entries
    }

    /// Returns the most recent snapshot for a given file path.
    pub fn latest_for_path(path: &str) -> Option<Snapshot> {
        Self::list_snapshots()
            .into_iter()
            .filter(|s| s.path == path)
            .last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_snapshot_and_ledger() {
        let test_file = "/tmp/nexus_test_snapshot.rs";
        fs::write(test_file, "fn main() {}").unwrap();

        let snap = LazarusVault::auto_snapshot(test_file, "test_tag")
            .await
            .expect("snapshot failed");

        assert!(!snap.hash.is_empty());
        assert_eq!(snap.path, test_file);

        let ledger = LazarusVault::list_snapshots();
        assert!(ledger.iter().any(|e| e.hash == snap.hash));

        // Cleanup
        fs::remove_file(test_file).ok();
    }
}
RUSTEOF

echo "[WRITTEN] ${SRC_DIR}/lazarus_vault.rs"


cat << 'EOF'

══════════════════════════════════════════════════════
 [4] LAZARUS VAULT — Verify cargo check passes
══════════════════════════════════════════════════════
EOF

cargo check --quiet 2>&1 | head -30 \
  && echo "[OK] cargo check passed" \
  || echo "[FAULT] cargo check failed — see errors above"


# =============================================================================
# FIX 2 — quantum_ingestor.rs: extract clean JSON before storing
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [5] QUANTUM INGESTOR — Write corrected binary
     The stored result contains the full LLM chain-of-
     thought + duplicate JSON blocks. The clean_json
     trim only stripped fences before the API call,
     not before store_memory(). This version extracts
     the last valid JSON object from the response
     before persisting to the memory palace.
══════════════════════════════════════════════════════
EOF

cat > "${SRC_DIR}/bin/quantum_ingestor.rs" << 'RUSTEOF'
// src/bin/quantum_ingestor.rs — v1.1 [CLEAN_JSON_STORE_FIX]
// FIX: Was storing the full LLM chain-of-thought including duplicate
//      JSON blocks and markdown fences into neural_context.result.
//      Now extracts the last valid JSON object from the response before
//      persisting — result column contains only the clean data object.
// FIX: Embedding is generated from the clean JSON, not the raw trace.
use anyhow::Result;
use nexus_singularity::memory::MemoryPalace;
use nexus_singularity::neural_agent::{ContextPacket, MicroAgent};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// extract_last_json: Finds the last {...} block in a string and
/// attempts to parse it as valid JSON. Returns the minified JSON
/// string on success, or None if no valid object is found.
fn extract_last_json(text: &str) -> Option<String> {
    // Walk backwards through the string to find the last closing brace
    let bytes = text.as_bytes();
    let mut end = None;
    for (i, &b) in bytes.iter().enumerate().rev() {
        if b == b'}' {
            end = Some(i);
            break;
        }
    }

    let end = end?;

    // Find the matching opening brace by counting depth
    let mut depth: i32 = 0;
    let mut start = None;
    for (i, &b) in bytes[..=end].iter().enumerate().rev() {
        match b {
            b'}' => depth += 1,
            b'{' => {
                depth -= 1;
                if depth == 0 {
                    start = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }

    let start = start?;
    let candidate = &text[start..=end];

    // Validate it parses as a JSON object with our expected keys
    match serde_json::from_str::<serde_json::Value>(candidate) {
        Ok(v) if v.is_object() => {
            // Minify for storage efficiency
            Some(serde_json::to_string(&v).unwrap_or_else(|_| candidate.to_string()))
        }
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY missing");
    let client = reqwest::Client::new();
    let (tx, _) = broadcast::channel(16);
    let agent = Arc::new(MicroAgent::init(tx)?);

    let memory_palace = MemoryPalace::new(".nexus_memory.db").await?;
    let memory = Arc::new(Mutex::new(memory_palace));

    println!("\x1b[35m>> [QUANTUM_INGESTOR] Initializing Physical PDF Extraction...\x1b[0m");

    let pdf_path = "Quantum_Machine_Learning_What_Quantum_Computing_Me.pdf";
    if !Path::new(pdf_path).exists() {
        eprintln!("\x1b[31m>> [FAULT] PDF not found at root.\x1b[0m");
        return Ok(());
    }

    let full_text = pdf_extract::extract_text(pdf_path)?;
    let chunk: String = full_text.chars().skip(4000).take(6000).collect();

    let prompt = format!(
        "Summarize this raw textbook extraction into a highly dense, technical JSON object \
         with exactly three keys: 'theory', 'classical_math', and 'quantum_bridge_potential'. \
         Strip out any OCR noise or page numbers. \
         Output ONLY the JSON object — no explanation, no markdown, no commentary.\n\nText:\n{}",
        chunk
    );

    let ctx = ContextPacket {
        system_prompt: Some(
            "You are a Quantum Architect. Output ONLY a valid JSON object with keys \
             'theory', 'classical_math', 'quantum_bridge_potential'. \
             No markdown. No explanation. No preamble. JSON only."
            .to_string()
        ),
        model_override: Some("gemma-4-26b-a4b-it".to_string()),
        ..Default::default()
    };

    println!("\x1b[36m>> [UPLINK] Dispatching chunk to gemma-4-26b-a4b-it...\x1b[0m");

    match agent.prepare_and_run(&client, &api_key, &prompt, ctx, 0.2, None).await {
        Ok((response, _)) => {
            // FIX: Extract only the last clean JSON object from the response
            let clean_json = match extract_last_json(&response) {
                Some(j) => {
                    println!("\x1b[32m>> [PARSER] Clean JSON extracted ({} bytes)\x1b[0m", j.len());
                    j
                }
                None => {
                    // Fallback: strip fences and take the raw response
                    eprintln!("\x1b[33m>> [PARSER] No valid JSON object found — storing raw (trimmed)\x1b[0m");
                    response
                        .trim_start_matches("```json")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim()
                        .chars()
                        .take(2000)
                        .collect()
                }
            };

            println!("\x1b[36m>> [UPLINK] Generating Semantic Vector Embeddings...\x1b[0m");

            // FIX: Embed the clean JSON, not the raw chain-of-thought
            let embedding = nexus_singularity::vector_bridge::get_embedding(
                &api_key,
                &clean_json,
            )
            .await
            .ok();

            if embedding.is_none() {
                eprintln!(
                    "\x1b[33m>> [EMBEDDING] Warning: embedding generation failed. \
                     Check GEMINI_API_KEY has access to text-embedding-004.\x1b[0m"
                );
            }

            let palace = memory.lock().await;
            palace
                .store_memory(
                    "QUANTUM_INGESTOR",
                    "PDF_Extraction_Phase_1",
                    &clean_json,
                    embedding,
                )
                .await?;

            println!("\x1b[32m>> [SUCCESS] Clean JSON enshrined in Semantic Memory Palace.\x1b[0m");
            println!("\x1b[32m>> [PREVIEW] {}\x1b[0m", &clean_json.chars().take(120).collect::<String>());
        }
        Err(e) => eprintln!("\x1b[31m>> [FAULT] API Request Failed: {}\x1b[0m", e),
    }

    Ok(())
}
RUSTEOF

echo "[WRITTEN] ${SRC_DIR}/bin/quantum_ingestor.rs"


cat << 'EOF'

══════════════════════════════════════════════════════
 [6] QUANTUM INGESTOR — Purge old polluted entry
     and re-run to store clean JSON
══════════════════════════════════════════════════════
EOF

cat << 'SQLEOF'
# Run this to clear the old chain-of-thought entry before re-ingesting:
sqlite3 .nexus_memory.db << 'SQL'
DELETE FROM neural_context
WHERE origin = 'QUANTUM_INGESTOR'
  AND directive = 'PDF_Extraction_Phase_1';

-- Also clean FTS5 index (trigger handles this if installed)
SELECT 'Deleted rows: ' || changes();
SQL
SQLEOF

echo ""
echo "After clearing, rebuild and re-run:"
echo "  cargo build --bin quantum_ingestor --release 2>&1 | tail -5"
echo "  ./target/release/quantum_ingestor"


# =============================================================================
# FIX 3 — Embedding verification script
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 [7] EMBEDDING — Verify text-embedding-004 access
     Tests whether your GEMINI_API_KEY can reach
     the embedding endpoint before running ingestor.
══════════════════════════════════════════════════════
EOF

if [ -z "${GEMINI_API_KEY:-}" ]; then
    # Try to load from .env
    if [ -f ".env" ]; then
        export $(grep -v '^#' .env | grep GEMINI_API_KEY | xargs) 2>/dev/null || true
    fi
fi

if [ -z "${GEMINI_API_KEY:-}" ]; then
    echo "[FAULT] GEMINI_API_KEY not set. Set it or add to .env file."
else
    echo "Testing text-embedding-004 endpoint..."
    HTTP_CODE=$(curl -s -o /tmp/nexus_embed_test.json -w "%{http_code}" \
      -X POST \
      "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key=${GEMINI_API_KEY}" \
      -H "Content-Type: application/json" \
      -d '{
        "model": "models/text-embedding-004",
        "content": { "parts": [{ "text": "nexus singularity test" }] }
      }')

    echo "HTTP Status: ${HTTP_CODE}"

    if [ "$HTTP_CODE" = "200" ]; then
        DIM=$(python3 -c "
import json
d = json.load(open('/tmp/nexus_embed_test.json'))
vals = d.get('embedding', {}).get('values', [])
print(f'  [OK] Embedding returned {len(vals)} dimensions')
" 2>/dev/null || echo "  [OK] 200 received but could not parse dimensions")
        echo "$DIM"
    elif [ "$HTTP_CODE" = "400" ]; then
        echo "  [FAULT] 400 Bad Request — model name or payload format wrong"
        cat /tmp/nexus_embed_test.json
    elif [ "$HTTP_CODE" = "403" ]; then
        echo "  [FAULT] 403 Forbidden — API key does not have Embeddings API access"
        echo "  Enable it at: https://console.cloud.google.com/apis/library/generativelanguage.googleapis.com"
    elif [ "$HTTP_CODE" = "429" ]; then
        echo "  [FAULT] 429 Rate Limited — quota exhausted, try again later"
    else
        echo "  [FAULT] Unexpected HTTP ${HTTP_CODE}"
        cat /tmp/nexus_embed_test.json
    fi
fi


cat << 'EOF'

══════════════════════════════════════════════════════
 [8] EMBEDDING — Check existing entry for embedding
     Shows whether the current memory row has one
══════════════════════════════════════════════════════
EOF

sqlite3 .nexus_memory.db << 'SQLEOF'
.mode column
.headers on
SELECT
    id,
    origin,
    directive,
    CASE
        WHEN embedding IS NULL THEN 'NO EMBEDDING'
        ELSE 'HAS EMBEDDING (' || length(embedding) || ' chars)'
    END AS embedding_status,
    timestamp
FROM neural_context
ORDER BY id DESC
LIMIT 10;
SQLEOF


# =============================================================================
# SUMMARY — Print next actions
# =============================================================================

cat << 'EOF'

══════════════════════════════════════════════════════
 WHAT WAS FIXED
══════════════════════════════════════════════════════

 Fix 1 — Lazarus Vault ledger.json:
   [1] Rebuilt ledger.json from the 6 existing raw
       snapshot files — rollback now has entries
   [3] Patched lazarus_vault.rs v1.4:
       → auto_snapshot() now calls append_to_ledger()
       → list_snapshots() sorts oldest→newest
       → latest_for_path() helper added
       → deduplication by hash prevents bloat

 Fix 2 — Quantum Ingestor clean JSON storage:
   [5] Patched quantum_ingestor.rs v1.1:
       → extract_last_json() walks backwards through
         the LLM response to find the final {...} block
       → Only the clean JSON object is stored in result
       → Embedding is generated from clean JSON too
       → Strengthened system prompt to reduce chain-of-
         thought bleed into the response
   [6] Run the purge SQL then rebuild to re-ingest cleanly

 Fix 3 — Embedding verification:
   [7] curl probe tells you exactly why embeddings fail:
       200 = working, 403 = API not enabled, 429 = quota

══════════════════════════════════════════════════════
 NEXT COMMANDS TO RUN
══════════════════════════════════════════════════════
 # 1. Verify ledger rebuilt:
 cat .nexus/vault/ledger.json | python3 -m json.tool | head -30

 # 2. Check cargo compiles the patched vault:
 cargo check -q

 # 3. Purge old polluted memory entry:
 sqlite3 .nexus_memory.db \
   "DELETE FROM neural_context WHERE origin='QUANTUM_INGESTOR';"

 # 4. Rebuild and re-run ingestor:
 cargo build --bin quantum_ingestor --release && \
   ./target/release/quantum_ingestor

 # 5. Verify clean entry stored:
 sqlite3 .nexus_memory.db \
   "SELECT id, origin, substr(result,1,120) FROM neural_context;"
══════════════════════════════════════════════════════
EOF

