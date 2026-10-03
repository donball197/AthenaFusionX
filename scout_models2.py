#!/usr/bin/env python3
"""
AthenaFusionX Model Scout — Full Manifest Edition (ANSI-safe)

- Reads models from .nexus_manifest.json (preferred) or ./models.py stdout.
- Strips ANSI color codes so parsed model IDs are clean.
- Tests OpenAI-compatible chat endpoints for Google, NVIDIA NIM, and local edge.
- Retries timeouts / 429 / 5xx with backoff.
- Tier-aware timeouts: HEAVY gets longer timeouts.
- Filters non-chat models by default unless --all is used.
- Generates .env.recommended based on latency + context size.
"""

import os
import sys
import re
import json
import time
import argparse
import subprocess
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed

CYAN   = "\033[1;36m"
GREEN  = "\033[1;32m"
YELLOW = "\033[1;33m"
RED    = "\033[1;31m"
BOLD   = "\033[1m"
DIM    = "\033[2m"
NC     = "\033[0m"

ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")


def strip_ansi(text: str) -> str:
    return ANSI_RE.sub("", text or "")


def load_env(env_path=".env"):
    config = {}
    if os.path.exists(env_path):
        with open(env_path, "r") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, v = line.split("=", 1)
                    config[k.strip()] = v.strip().strip('"').strip("'")
    return config


ENV = load_env()

GOOGLE_KEY = ENV.get("GEMINI_API_KEY") or ENV.get("GOOGLE_API_KEY", "")
NVIDIA_KEY = ENV.get("NVIDIA_API_KEY", "")
EDGE_GW    = ENV.get("OPENAI_BASE_URL") or ENV.get("NEXUS_API_GATEWAY", "http://192.168.4.223:9379/v1")
EMBED_URL  = ENV.get("NEXUS_EMBEDDING_URL", "http://127.0.0.1:9380/v1/embeddings")


def parse_context_tokens(tokens):
    """Parse '128K in / 4K out' -> 128000. Unknown/N/A -> 0."""
    if not tokens:
        return 0
    m = re.search(r"([\d.]+)\s*([KMB]?)\s*in", tokens, re.I)
    if not m:
        return 0
    num = float(m.group(1))
    unit = m.group(2).upper()
    mult = {"": 1, "K": 1000, "M": 1_000_000, "B": 1_000_000_000}.get(unit, 1)
    return int(num * mult)


def infer_tier(category, model, context):
    cat = (category or "").upper()
    mod = (model or "").lower()

    if "EMBED" in cat:
        return "EMBEDDING"
    if "TTS" in cat or "IMAGE" in cat:
        return "NON_CHAT"
    if "FLASH" in cat or "PLANNER" in cat:
        return "PLANNER"
    if "PRO" in cat or "HEAVY" in cat or "NEMOTRON" in cat or "MISTRAL" in cat or "LLAMA" in cat:
        return "HEAVY"
    if "GEMMA" in cat:
        return "EDGE" if context and context <= 32768 else "HEAVY"
    if "VISION" in cat:
        return "VISION"
    return "GENERAL"


def is_chat_capable(category, model, context):
    """Skip image/TTS/embedding models unless --all is used."""
    cat = (category or "").upper()
    if "EMBED" in cat or "TTS" in cat or "IMAGE" in cat:
        return False
    return True


def run_manifest(manifest_cmd="./models.py"):
    """Run models.py and return a clean list of model dicts.

    Preferred path: read .nexus_manifest.json (written by models.py).
    Fallback:       parse stdout with ANSI codes stripped.
    """
    if os.path.isfile(manifest_cmd) and os.access(manifest_cmd, os.X_OK):
        cmd = [manifest_cmd]
    elif os.path.isfile(manifest_cmd):
        cmd = [sys.executable, manifest_cmd]
    else:
        cmd = manifest_cmd.split()

    try:
        subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    except Exception as e:
        print(f"{RED}Failed to run manifest command: {e}{NC}")
        return []

    # --- Preferred: JSON manifest ---
    json_path = ".nexus_manifest.json"
    if os.path.exists(json_path):
        try:
            with open(json_path, "r", encoding="utf-8") as f:
                data = json.load(f)
            models = []
            for m in data:
                provider = m.get("provider", "")
                model_id = m.get("name", "")
                tokens   = m.get("context", "Unknown")
                category = m.get("category", "GENERAL")
                if not provider or not model_id:
                    continue
                context = parse_context_tokens(tokens)
                tier = infer_tier(category, model_id, context)
                models.append({
                    "provider": provider,
                    "model": model_id,
                    "tokens": tokens,
                    "category": category,
                    "context": context,
                    "tier": tier,
                })
            if models:
                print(f"{DIM}Loaded {len(models)} models from {json_path}{NC}")
                return models
        except Exception as e:
            print(f"{YELLOW}JSON manifest parse failed, falling back to stdout: {e}{NC}")

    # --- Fallback: parse stdout, stripping ANSI ---
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    except Exception as e:
        print(f"{RED}Failed to re-run manifest command: {e}{NC}")
        return []

    models = []
    for raw_line in proc.stdout.splitlines():
        line = strip_ansi(raw_line).strip()
        if not line or line.startswith("PROVIDER") or line.startswith("---"):
            continue
        if "|" not in line:
            continue

        parts = [p.strip() for p in line.split("|")]
        if len(parts) < 4:
            continue

        provider, model_id, tokens, category = parts[0], parts[1], parts[2], parts[3]
        if not provider or not model_id:
            continue

        context = parse_context_tokens(tokens)
        tier = infer_tier(category, model_id, context)

        models.append({
            "provider": provider,
            "model": model_id,
            "tokens": tokens,
            "category": category,
            "context": context,
            "tier": tier,
        })

    return models


def provider_endpoint(provider):
    p = (provider or "").lower()
    if "google" in p:
        return (
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            GOOGLE_KEY,
        )
    if "nvidia" in p:
        return (
            "https://integrate.api.nvidia.com/v1/chat/completions",
            NVIDIA_KEY,
        )
    if "local" in p or "edge" in p or "s23" in p:
        return (EDGE_GW.rstrip("/") + "/chat/completions", "")
    return (None, None)


def probe_chat_model(provider, model_id, tier, url, api_key,
                     timeout=45, retries=1, probes=3, window=8.0):
    import statistics
    ok_count = 0
    latencies = []
    last_err = ""
    spacing = window / max(probes - 1, 1) if probes > 1 else 0.0
    for i in range(probes):
        r = test_chat_model(provider, model_id, tier, url, api_key, timeout, retries)
        if r["ok"]:
            ok_count += 1
            latencies.append(r["latency"])
        elif r["error"]:
            last_err = r["error"]
        if i < probes - 1 and spacing > 0:
            time.sleep(spacing)
    if ok_count == probes:
        stab = "STABLE"
    elif ok_count > 0:
        stab = "FLAKY"
    else:
        stab = "DOWN"
    p50 = int(statistics.median(latencies)) if latencies else 0
    agg = {
        "provider": provider,
        "model": model_id,
        "tier": tier,
        "status": 200 if ok_count else 0,
        "latency": p50,
        "ok": ok_count > 0,
        "error": last_err[:120] if ok_count < probes else "",
        "stability": stab,
        "probe_successes": ok_count,
        "probe_attempts": probes,
    }
    return agg


def test_chat_model(provider, model_id, tier, url, api_key, timeout=45, retries=1):
    payload = json.dumps({
        "model": model_id,
        "messages": [{"role": "user", "content": "Ping"}],
        "max_tokens": 1,
        "temperature": 0.0,
    }).encode("utf-8")

    last_error = ""
    for attempt in range(retries + 1):
        req = urllib.request.Request(
            url,
            data=payload,
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Bearer {api_key}" if api_key else "",
            },
        )

        t0 = time.time()
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                elapsed_ms = int((time.time() - t0) * 1000)
                return {
                    "provider": provider,
                    "model": model_id,
                    "tier": tier,
                    "status": resp.getcode(),
                    "latency": elapsed_ms,
                    "ok": True,
                    "error": "",
                }
        except urllib.error.HTTPError as e:
            elapsed_ms = int((time.time() - t0) * 1000)
            last_error = f"HTTP {e.code}"
            if e.code in (429, 500, 502, 503, 504) and attempt < retries:
                time.sleep(1.5 ** attempt)
                continue
            return {
                "provider": provider,
                "model": model_id,
                "tier": tier,
                "status": e.code,
                "latency": elapsed_ms,
                "ok": False,
                "error": last_error,
            }
        except Exception as e:
            elapsed_ms = int((time.time() - t0) * 1000)
            last_error = str(e)[:120]
            if attempt < retries:
                time.sleep(1.5 ** attempt)
                continue
            return {
                "provider": provider,
                "model": model_id,
                "tier": tier,
                "status": 0,
                "latency": elapsed_ms,
                "ok": False,
                "error": last_error,
            }

    return {
        "provider": provider,
        "model": model_id,
        "tier": tier,
        "status": 0,
        "latency": 0,
        "ok": False,
        "error": last_error,
    }


def test_embedding(url, model_name="granite-311m-edge"):
    payload = json.dumps({
        "input": "scout embedding check",
        "model": model_name,
    }).encode("utf-8")

    req = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json"},
    )
    t0 = time.time()
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            elapsed_ms = int((time.time() - t0) * 1000)
            data = json.loads(resp.read().decode("utf-8"))
            dim = len(data["data"][0]["embedding"])
            return True, elapsed_ms, f"{dim}-dim"
    except Exception as e:
        return False, 0, str(e)[:120]


def main():
    parser = argparse.ArgumentParser(description="AthenaFusionX full model scout")
    parser.add_argument("--manifest-cmd", default="./models.py", help="Command to generate model manifest")
    parser.add_argument("--all", action="store_true", help="Test non-chat models too (image/TTS/embedding)")
    parser.add_argument("--timeout", type=int, default=0, help="Override timeout for all models")
    parser.add_argument("--retries", type=int, default=1, help="Retries for timeout/429/5xx")
    parser.add_argument("--max-workers", type=int, default=10, help="Parallel workers")
    parser.add_argument("--probes", type=int, default=3, help="Pings per model to detect flapping")
    parser.add_argument("--probe-window", type=float, default=8.0, help="Seconds between pings")
    parser.add_argument("--json", action="store_true", help="Dump JSON results")
    args = parser.parse_args()

    print(f"\n{CYAN}========================================================================{NC}")
    print(f"{BOLD}>> ATHENAFUSIONX FULL MODEL SCOUT — MANIFEST AUDIT{NC}")
    print(f"{CYAN}========================================================================{NC}")

    manifest = run_manifest(args.manifest_cmd)
    if not manifest:
        print(f"{RED}No models found. Check --manifest-cmd or ./models.py{NC}")
        sys.exit(1)

    # Add local edge model if not already in manifest
    edge_model = ENV.get("NEXUS_LIGHT_MODEL", "gemma-4-e4b-it")
    if not any(m["model"] == edge_model for m in manifest):
        manifest.append({
            "provider": "Local S23",
            "model": edge_model,
            "tokens": "Unknown",
            "category": "EDGE",
            "context": 0,
            "tier": "EDGE",
        })

    if not args.all:
        manifest = [m for m in manifest if is_chat_capable(m["category"], m["model"], m["context"])]

    print(f"Scouting {BOLD}{len(manifest)}{NC} models from manifest...\n")

    tasks = []
    for m in manifest:
        url, key = provider_endpoint(m["provider"])
        if not url:
            continue
        tier = m["tier"]
        if args.timeout:
            timeout = args.timeout
        elif tier == "HEAVY":
            timeout = 90
        elif tier == "PLANNER":
            timeout = 35
        else:
            timeout = 45
        tasks.append((m, url, key, timeout))

    results = []
    with ThreadPoolExecutor(max_workers=args.max_workers) as executor:
        futures = [
            executor.submit(
                probe_chat_model,
                m["provider"],
                m["model"],
                m["tier"],
                url,
                key,
                timeout,
                args.retries,
                args.probes,
                args.probe_window,
            )
            for m, url, key, timeout in tasks
        ]
        for f in as_completed(futures):
            r = f.result()
            for m in manifest:
                if m["model"] == r["model"] and m["provider"] == r["provider"]:
                    r["context"] = m["context"]
                    r["tokens"] = m["tokens"]
                    r["category"] = m["category"]
                    break
            results.append(r)

    results.sort(key=lambda r: (not r["ok"], r["latency"]))

    print(f"{'STATUS':<12} | {'PROVIDER':<10} | {'LATENCY':<8} | {'CTX':<8} | {'TIER':<9} | {'MODEL ID'}")
    print("-" * 110)

    working = []
    for r in results:
        if r["ok"]:
            working.append(r)
            status_badge = f"{GREEN}200 OK{NC}      "
            lat_str = f"{r['latency']}ms"
            ctx_str = f"{r.get('context', 0) // 1000}K" if r.get("context") else "?"
            print(f"{status_badge} | {r['provider']:<10} | {lat_str:<8} | {ctx_str:<8} | {r['tier']:<9} | {BOLD}{r['model']}{NC}")
        else:
            status_badge = f"{RED}{r['error']:<11}{NC}"
            ctx_str = f"{r.get('context', 0) // 1000}K" if r.get("context") else "?"
            print(f"{status_badge} | {r['provider']:<10} | {'---':<8} | {ctx_str:<8} | {r['tier']:<9} | {DIM}{r['model']}{NC}")

    print("-" * 110)

    embed_ok, embed_lat, embed_info = test_embedding(EMBED_URL)
    if embed_ok:
        print(f"{GREEN}200 OK{NC}      | Duet Edge  | {embed_lat}ms    | {'?':<8} | EMBEDDING | {BOLD}granite-311m-edge ({embed_info}){NC}")
    else:
        print(f"{RED}OFFLINE{NC}     | Duet Edge  | ---      | {'?':<8} | EMBEDDING | {DIM}granite-311m-edge ({embed_info}){NC}")

    print(f"{CYAN}========================================================================{NC}")
    print(f"Summary: {GREEN}{len(working)} models verified active{NC} / {len(results) - len(working)} unavailable.\n")

    # --- Recommendations ---
    planners = [
        r for r in working
        if r["tier"] in ("PLANNER", "GENERAL") and r.get("context", 0) >= 128000
    ]
    if not planners:
        planners = [r for r in working if r["tier"] in ("PLANNER", "GENERAL")]
    stable_ok = [r for r in working if r.get("stability") == "STABLE"]
    if stable_ok:
        planners = [r for r in planners if r in stable_ok] or planners
    planners.sort(key=lambda r: r["latency"])

    heavies = [
        r for r in working
        if r["tier"] in ("HEAVY", "PRO", "GENERAL") and r.get("context", 0) >= 128000
    ]
    if not heavies:
        heavies = [r for r in working if r["tier"] in ("HEAVY", "PRO", "GENERAL")]
    if stable_ok:
        heavies = [r for r in heavies if r in stable_ok] or heavies
    heavies.sort(key=lambda r: (-r.get("context", 0), r["latency"]))

    edges = [r for r in working if r["tier"] == "EDGE"]
    edges.sort(key=lambda r: r["latency"])

    best_planner        = planners[0]["model"] if planners else "gemini-3.5-flash-lite"
    best_planner_cascade = planners[1]["model"] if len(planners) > 1 else best_planner
    best_heavy          = heavies[0]["model"] if heavies else "gemini-3.5-flash"
    best_heavy_cascade  = heavies[1]["model"] if len(heavies) > 1 else best_heavy
    best_edge           = edges[0]["model"] if edges else "gemma-4-e4b-it"

    print(f"{BOLD}>> RECOMMENDED ATHENA TIER CONFIGURATION:{NC}")
    print(f"  • {BOLD}Planner (Fast DAG Orchestration):{NC} {CYAN}{best_planner}{NC} (Fallback: {best_planner_cascade})")
    print(f"  • {BOLD}Heavy   (Deep Code & Reasoning):{NC} {CYAN}{best_heavy}{NC} (Fallback: {best_heavy_cascade})")
    print(f"  • {BOLD}Light   (Local Edge Node):{NC}       {CYAN}{best_edge}{NC}")
    print(f"  • {BOLD}Embed   (Semantic Memory):{NC}       {CYAN}granite-311m-edge{NC}")

    rec_file = ".env.recommended"
    with open(rec_file, "w") as f:
        f.write("# Automatically generated by improved scout_models2.py\n")
        f.write(f"NEXUS_PLANNER_MODEL={best_planner}\n")
        f.write(f"NEXUS_PLANNER_CASCADE={best_planner_cascade}\n")
        f.write(f"NEXUS_HEAVY_MODEL={best_heavy}\n")
        f.write(f"NEXUS_HEAVY_CASCADE={best_heavy_cascade}\n")
        f.write(f"NEXUS_LIGHT_MODEL={best_edge}\n")
        f.write(f"NEXUS_LIGHT_CASCADE={best_planner}\n")
        f.write(f"NEXUS_EMBEDDING_MODEL=granite-311m-edge\n")
        f.write(f"NEXUS_EMBEDDING_URL={EMBED_URL}\n")

    print(f"\n{GREEN}✔ Generated recommended tier settings to: {rec_file}{NC}")

    if args.json:
        with open("scout_results.json", "w") as f:
            json.dump(results, f, indent=2)
        print(f"{GREEN}✔ JSON results written to: scout_results.json{NC}")


if __name__ == "__main__":
    main()
