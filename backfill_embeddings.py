#!/usr/bin/env python3
"""Backfill neural_context embeddings via the local gateway. Idempotent; safe to re-run."""
import argparse, json, os, sqlite3, sys, urllib.request

MODEL, DIM, MAX_ATTEMPTS = "granite-embedding-311m-r2", 768, 8
URL = os.environ.get("NEXUS_EMBEDDING_URL", "http://127.0.0.1:9380/v1/embeddings")
COLS = {
    "embedding_status": "TEXT NOT NULL DEFAULT 'pending'",
    "embedding_model": "TEXT NULL",
    "embedding_error": "TEXT NULL",
    "embedding_attempts": "INTEGER NOT NULL DEFAULT 0",
}

def embed(text):
    req = urllib.request.Request(URL, json.dumps({"input": text}).encode(),
                                 {"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        v = json.load(r)["data"][0]["embedding"]
    if len(v) != DIM:
        raise RuntimeError(f"got {len(v)} dims, expected {DIM}")
    return v

def source_text(origin, directive, result):
    # Mirrors the live callers: genomes embed the directive, everything else the first 4000 chars.
    return directive if origin == "SovereignGenome" else result[:4000]

ap = argparse.ArgumentParser()
ap.add_argument("db")
ap.add_argument("--retry-failed", action="store_true")
ap.add_argument("--probe", help="embed this query and print the 5 nearest stored rows")
a = ap.parse_args()

if not os.path.exists(a.db):
    sys.exit(f"db not found: {a.db}")
con = sqlite3.connect(a.db, timeout=30)
have = {r[1] for r in con.execute("PRAGMA table_info(neural_context)")}
if not have:
    sys.exit("table neural_context not found in this db")
for c, d in COLS.items():
    if c not in have:
        con.execute(f"ALTER TABLE neural_context ADD COLUMN {c} {d}")
if a.retry_failed:
    con.execute("UPDATE neural_context SET embedding_status='pending', embedding_attempts=0 WHERE embedding_status='failed'")
con.commit()

rows = con.execute("SELECT id, origin, directive, result FROM neural_context "
                   "WHERE embedding_status='pending' ORDER BY id").fetchall()
print(f"{len(rows)} pending", flush=True)
ok = fail = streak = 0
for n, (id_, origin, directive, result) in enumerate(rows, 1):
    try:
        v = embed(source_text(origin, directive, result))
        con.execute("UPDATE neural_context SET embedding=?, embedding_status='ready', "
                    "embedding_model=?, embedding_error=NULL WHERE id=?", (json.dumps(v), MODEL, id_))
        ok += 1; streak = 0
    except Exception as e:
        fail += 1; streak += 1
        con.execute("UPDATE neural_context SET embedding_attempts=embedding_attempts+1, embedding_error=?, "
                    "embedding_status=CASE WHEN embedding_attempts+1>=? THEN 'failed' ELSE 'pending' END "
                    "WHERE id=?", (f"{type(e).__name__}: {e}"[:500], MAX_ATTEMPTS, id_))
        if streak >= 5:
            con.commit()
            sys.exit("gateway looks down (5 consecutive failures); aborting")
    if n % 25 == 0:
        con.commit(); print(f"{n}/{len(rows)} ok={ok} fail={fail}", flush=True)
con.commit()
print(f"done ok={ok} fail={fail}")

if a.probe:
    q = embed(a.probe)
    scored = []
    for id_, origin, directive, emb in con.execute(
            "SELECT id, origin, directive, embedding FROM neural_context WHERE embedding_status='ready'"):
        scored.append((sum(x * y for x, y in zip(q, json.loads(emb))), id_, origin, directive[:70]))
    for s, *rest in sorted(scored, reverse=True)[:5]:
        print(f"{s:.3f}", *rest)
