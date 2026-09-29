#!/usr/bin/env python3
"""AthenaFusionX embedding gateway: real Granite via LiteRT-LM. No fake vectors."""
import json, math, os, sys, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODEL_PATH = os.environ.get("NEXUS_EMBED_MODEL_PATH",
    "/root/.litert-lm/models/granite-embedding-311m-r2_wi8fc/model.litertlm")
MODEL_ID   = os.environ.get("NEXUS_EMBED_MODEL_ID", "granite-embedding-311m-r2")
HOST       = os.environ.get("NEXUS_EMBED_HOST", "127.0.0.1")
PORT       = int(os.environ.get("NEXUS_EMBED_PORT", "9380"))
EXPECT_DIM = int(os.environ.get("NEXUS_EMBEDDING_DIM", "768"))
OVERFLOW   = os.environ.get("NEXUS_EMBED_OVERFLOW", "TRUNCATE").upper()
MARGIN     = float(os.environ.get("NEXUS_EMBED_SELFTEST_MARGIN", "0.05"))

ENGINE = None
OPTS = None
LOAD_ERROR = None
LOCK = threading.Lock()

def cos(a, b):
    d = sum(x * y for x, y in zip(a, b))
    return d / (math.sqrt(sum(x * x for x in a)) * math.sqrt(sum(y * y for y in b)))

def embed(texts):
    out = []
    with LOCK:
        for t in texts:
            t = t if t.strip() else "[empty]"   # deterministic placeholder for empty input
            v = [float(x) for x in ENGINE.compute_embedding(t, OPTS).embedding]
            if len(v) != EXPECT_DIM:
                raise RuntimeError(f"model returned {len(v)} dims, expected {EXPECT_DIM}")
            out.append(v)
    return out

def selftest():
    a, b, c = embed(["The cat sat on the mat.",
                     "A kitten is resting on a rug.",
                     "The bank approved the mortgage loan."])
    a2 = embed(["The cat sat on the mat."])[0]
    rel, unrel, det = cos(a, b), cos(a, c), cos(a, a2)
    print(f"[SELFTEST] dim={len(a)} related={rel:.4f} unrelated={unrel:.4f} "
          f"margin={rel-unrel:.4f} determinism={det:.6f}", flush=True)
    if det < 0.9999:
        raise RuntimeError("non-deterministic embeddings")
    if rel - unrel < MARGIN:
        raise RuntimeError(f"semantic margin {rel-unrel:.4f} < {MARGIN}")
    embed(["fn main() { println!(\"hi\"); } " * 200])  # ~6000 chars must not error
    print("[SELFTEST] long-input overflow handling OK", flush=True)

def load():
    global ENGINE, OPTS, LOAD_ERROR
    try:
        from litert_lm.embedding_engine import (EmbeddingEngine, EmbeddingOptions,
                                                InputOverflowStrategy)
        OPTS = EmbeddingOptions(normalize=True,
                                input_overflow_strategy=InputOverflowStrategy[OVERFLOW])
        ENGINE = EmbeddingEngine(MODEL_PATH)
        selftest()
    except Exception as e:
        ENGINE = None
        LOAD_ERROR = f"{type(e).__name__}: {e}"
        print(f"[ATHENA-EDGE] MODEL NOT SERVING: {LOAD_ERROR}", file=sys.stderr, flush=True)

class H(BaseHTTPRequestHandler):
    def _send(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/health":
            ok = ENGINE is not None
            self._send(200 if ok else 503, {
                "status": "online" if ok else "degraded", "engine": "LiteRT-LM",
                "model": MODEL_ID, "dimension": EXPECT_DIM, "overflow": OVERFLOW,
                "error": LOAD_ERROR})
        elif self.path == "/v1/models":
            self._send(200, {"object": "list", "data": [
                {"id": MODEL_ID, "object": "model", "type": "embedding"}]})
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self):
        if self.path != "/v1/embeddings":
            return self._send(404, {"error": "not found"})
        if ENGINE is None:
            return self._send(503, {"error": f"embedding model not loaded: {LOAD_ERROR}"})
        try:
            n = int(self.headers.get("Content-Length", 0))
            req = json.loads(self.rfile.read(n) or b"{}")
            inp = req.get("input")
            texts = [inp] if isinstance(inp, str) else inp
            if not isinstance(texts, list) or not all(isinstance(t, str) for t in texts):
                return self._send(400, {"error": "input must be a string or list of strings"})
            vecs = embed(texts)
            self._send(200, {"object": "list", "model": MODEL_ID,
                "data": [{"object": "embedding", "index": i, "embedding": v}
                         for i, v in enumerate(vecs)]})
        except Exception as e:
            self._send(500, {"error": f"{type(e).__name__}: {e}"})

    def log_message(self, *a):
        pass

if __name__ == "__main__":
    load()
    print(f"[ATHENA-EDGE] {MODEL_ID} dim={EXPECT_DIM} on http://{HOST}:{PORT} "
          f"({'READY' if ENGINE else 'DEGRADED'})", flush=True)
    ThreadingHTTPServer((HOST, PORT), H).serve_forever()
