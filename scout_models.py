#!/usr/bin/env python3
"""
AthenaFusionX — Curated Active Model Scout

Tests ONLY the currently verified-active model set supplied by the
AthenaFusionX operator.

No manifest discovery.
No stale model catalog.
No automatic promotion of arbitrary models.

Roles are explicit so model discovery cannot silently change the
AthenaFusionX architecture.
"""

import os
import json
import time
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed

CYAN = "\033[1;36m"
GREEN = "\033[1;32m"
YELLOW = "\033[1;33m"
RED = "\033[1;31m"
BOLD = "\033[1m"
DIM = "\033[2m"
NC = "\033[0m"


# ---------------------------------------------------------------------------
# ENV
# ---------------------------------------------------------------------------

def load_env(env_path=".env"):
    config = {}

    if os.path.exists(env_path):
        with open(env_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()

                if not line or line.startswith("#") or "=" not in line:
                    continue

                key, value = line.split("=", 1)
                config[key.strip()] = value.strip().strip('"').strip("'")

    return config


ENV = load_env()

GOOGLE_KEY = (
    ENV.get("GEMINI_API_KEY")
    or ENV.get("GOOGLE_API_KEY")
    or ""
)

NVIDIA_KEY = ENV.get("NVIDIA_API_KEY", "")

EDGE_GW = (
    ENV.get("OPENAI_BASE_URL")
    or ENV.get("NEXUS_API_GATEWAY")
    or "http://192.168.4.223:9379/v1"
)

EMBED_URL = ENV.get(
    "NEXUS_EMBEDDING_URL",
    "http://127.0.0.1:9380/v1/embeddings",
)


# ---------------------------------------------------------------------------
# FIXED ACTIVE MODEL SET
#
# These are the models from the operator-supplied active audit.
# Do not add manifest models here automatically.
# ---------------------------------------------------------------------------

CANDIDATES = [

    # =======================================================================
    # GOOGLE — VERIFIED ACTIVE
    # =======================================================================

    (
        "Google",
        "gemini-3.5-transcribe",
        "GENERAL",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemma-4-26b-a4b-it",
        "HEAVY",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-2.5-flash-lite",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-flash-lite-latest",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3.1-flash-lite",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3.1-flash-lite-preview",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3.5-flash-lite",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3.6-flash",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-robotics-er-2-preview",
        "GENERAL",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3-flash-preview",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3.7-flash",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3.8-flash",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-flash-latest",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemma-4-31b-it",
        "HEAVY",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),

    (
        "Google",
        "gemini-3.5-flash",
        "PLANNER",
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        GOOGLE_KEY,
    ),


    # =======================================================================
    # NVIDIA — VERIFIED ACTIVE
    # =======================================================================

    (
        "Nvidia",
        "nvidia/nemotron-3.5-content-safety",
        "GENERAL",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "nvidia/nemotron-parse-2.0",
        "HEAVY",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "nvidia/riva-translate-4b-instruct-v1.1",
        "GENERAL",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "nvidia/riva-translate-4b-instruct-v2",
        "GENERAL",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "nvidia/llama-3.1-nemotron-safety-guard-8b-v3",
        "GENERAL",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "openai/gpt-oss-20b",
        "GENERAL",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "mistralai/mistral-nemotron",
        "HEAVY",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "meta/llama-3.2-11b-vision-instruct",
        "VISION",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "meta/muse-glimmer-30b",
        "GENERAL",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "nvidia/nemotron-3-super-120b-a12b",
        "HEAVY",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning",
        "HEAVY",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "nvidia/ising-calibration-1.5-31b",
        "HEAVY",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "z-ai/glm-5.3-flash",
        "HEAVY",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),

    (
        "Nvidia",
        "deepseek-ai/deepseek-v4-pro-0813",
        "HEAVY",
        "https://integrate.api.nvidia.com/v1/chat/completions",
        NVIDIA_KEY,
    ),


    # =======================================================================
    # LOCAL S23 — VERIFIED ACTIVE
    # =======================================================================

    (
        "Local S23",
        "gemma-4-e4b-it",
        "EDGE",
        f"{EDGE_GW.rstrip('/')}/chat/completions",
        "local-s23-device",
    ),
]


# ---------------------------------------------------------------------------
# TEST
# ---------------------------------------------------------------------------

def test_chat_model(
    provider,
    model_id,
    target_tier,
    url,
    api_key,
    timeout=60,
):
    payload = json.dumps({
        "model": model_id,
        "messages": [
            {
                "role": "user",
                "content": "Ping",
            }
        ],
        "max_tokens": 8,
        "temperature": 0.0,
    }).encode("utf-8")

    headers = {
        "Content-Type": "application/json",
    }

    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"

    req = urllib.request.Request(
        url,
        data=payload,
        headers=headers,
    )

    t0 = time.time()

    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            elapsed_ms = int((time.time() - t0) * 1000)

            # We intentionally do not require non-empty content.
            # Reasoning-capable models may consume a tiny token budget
            # without returning normal assistant content.
            return {
                "provider": provider,
                "model": model_id,
                "tier": target_tier,
                "status": resp.getcode(),
                "latency": elapsed_ms,
                "ok": resp.getcode() == 200,
                "error": "",
            }

    except urllib.error.HTTPError as e:
        elapsed_ms = int((time.time() - t0) * 1000)

        return {
            "provider": provider,
            "model": model_id,
            "tier": target_tier,
            "status": e.code,
            "latency": elapsed_ms,
            "ok": False,
            "error": f"HTTP {e.code}",
        }

    except Exception as e:
        elapsed_ms = int((time.time() - t0) * 1000)

        return {
            "provider": provider,
            "model": model_id,
            "tier": target_tier,
            "status": 0,
            "latency": elapsed_ms,
            "ok": False,
            "error": str(e),
        }


# ---------------------------------------------------------------------------
# EMBEDDING
# ---------------------------------------------------------------------------

def test_embedding(
    url,
    model_name="granite-311m-edge",
):
    payload = json.dumps({
        "input": "AthenaFusionX semantic memory probe",
        "model": model_name,
    }).encode("utf-8")

    req = urllib.request.Request(
        url,
        data=payload,
        headers={
            "Content-Type": "application/json",
        },
    )

    t0 = time.time()

    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            elapsed_ms = int((time.time() - t0) * 1000)

            data = json.loads(
                resp.read().decode("utf-8")
            )

            dim = len(
                data["data"][0]["embedding"]
            )

            return True, elapsed_ms, f"{dim}-dim"

    except Exception as e:
        return False, 0, str(e)


# ---------------------------------------------------------------------------
# CONFIGURATION
# ---------------------------------------------------------------------------

def write_recommended_env(working_models):
    """
    Writes only the AthenaFusionX role configuration.

    The architecture is explicit:
      Planner -> Gemini
      Heavy   -> GLM
      Light   -> S23 Gemma E4B
      Embed   -> Granite 311M

    We do NOT allow latency sorting to silently change these roles.
    """

    available = {
        m["model"]
        for m in working_models
    }

    planner_primary = "gemini-3.5-flash-lite"
    planner_fallback = "gemini-2.5-flash-lite"

    heavy_primary = "z-ai/glm-5.3-flash"
    heavy_fallback = "nvidia/ising-calibration-1.5-31b"

    edge_primary = "gemma-4-e4b-it"

    # Keep only verified active models as fallback.
    if planner_primary not in available:
        planner_primary = "gemini-2.5-flash-lite"

    if planner_fallback not in available:
        planner_fallback = planner_primary

    if heavy_primary not in available:
        heavy_primary = "deepseek-ai/deepseek-v4-pro-0813"

    if heavy_fallback not in available:
        heavy_fallback = heavy_primary

    if edge_primary not in available:
        edge_primary = "gemma-4-e4b-it"

    rec_file = ".env.recommended"

    with open(rec_file, "w", encoding="utf-8") as f:
        f.write(
            "# AthenaFusionX curated active model configuration\n"
        )
        f.write(
            "# Generated only from the operator-approved active model set.\n"
        )
        f.write("\n")

        f.write(
            f"NEXUS_PLANNER_MODEL={planner_primary}\n"
        )
        f.write(
            f"NEXUS_PLANNER_CASCADE={planner_primary},{planner_fallback}\n"
        )
        f.write("\n")

        f.write(
            f"NEXUS_HEAVY_MODEL={heavy_primary}\n"
        )
        f.write(
            f"NEXUS_HEAVY_CASCADE={heavy_primary},{heavy_fallback}\n"
        )
        f.write("\n")

        f.write(
            f"NEXUS_LIGHT_MODEL={edge_primary}\n"
        )
        f.write(
            f"NEXUS_LIGHT_CASCADE={edge_primary}\n"
        )
        f.write("\n")

        f.write(
            "NEXUS_EMBEDDING_MODEL=granite-311m-edge\n"
        )
        f.write(
            f"NEXUS_EMBEDDING_URL={EMBED_URL}\n"
        )

    return rec_file


# ---------------------------------------------------------------------------
# MAIN
# ---------------------------------------------------------------------------

def main():
    print()
    print(f"{CYAN}{'=' * 72}{NC}")
    print(
        f"{BOLD}>> ATHENAFUSIONX CURATED MODEL SCOUT — ACTIVE SET ONLY{NC}"
    )
    print(f"{CYAN}{'=' * 72}{NC}")
    print()
    print(
        f"Testing {BOLD}{len(CANDIDATES)}{NC} "
        "operator-approved active models only..."
    )
    print()

    results = []

    # Four workers keeps the Android/Termux workload bounded.
    with ThreadPoolExecutor(max_workers=4) as executor:
        futures = [
            executor.submit(
                test_chat_model,
                *candidate,
            )
            for candidate in CANDIDATES
        ]

        for future in as_completed(futures):
            results.append(future.result())

    # Fastest successful models first, then failures.
    results.sort(
        key=lambda r: (
            not r["ok"],
            r["latency"] if r["ok"] else 999999,
        )
    )

    print(
        f"{'STATUS':<12} | "
        f"{'PROVIDER':<10} | "
        f"{'LATENCY':<9} | "
        f"{'TIER':<9} | "
        f"MODEL ID"
    )
    print("-" * 100)

    working_models = []

    for r in results:
        if r["ok"]:
            working_models.append(r)

            status = f"{GREEN}{r['status']} OK{NC}"
            latency = f"{r['latency']}ms"

            print(
                f"{status:<21} | "
                f"{r['provider']:<10} | "
                f"{latency:<9} | "
                f"{r['tier']:<9} | "
                f"{BOLD}{r['model']}{NC}"
            )

        else:
            if r["status"]:
                status_text = f"HTTP {r['status']}"
            else:
                status_text = r["error"] or "FAILED"

            print(
                f"{RED}{status_text:<12}{NC} | "
                f"{r['provider']:<10} | "
                f"{'---':<9} | "
                f"{r['tier']:<9} | "
                f"{DIM}{r['model']}{NC}"
            )

    print("-" * 100)

    # -----------------------------------------------------------------------
    # EMBEDDING
    # -----------------------------------------------------------------------

    embed_ok, embed_lat, embed_info = test_embedding(
        EMBED_URL
    )

    if embed_ok:
        print(
            f"{GREEN}200 OK{NC}       | "
            f"{'Duet Edge':<10} | "
            f"{str(embed_lat) + 'ms':<9} | "
            f"{'EMBEDDING':<9} | "
            f"{BOLD}granite-311m-edge ({embed_info}){NC}"
        )
    else:
        print(
            f"{RED}OFFLINE{NC}      | "
            f"{'Duet Edge':<10} | "
            f"{'---':<9} | "
            f"{'EMBEDDING':<9} | "
            f"{DIM}granite-311m-edge ({embed_info}){NC}"
        )

    print("-" * 100)

    unavailable = len(CANDIDATES) - len(working_models)

    print(
        f"Summary: "
        f"{GREEN}{len(working_models)} verified active{NC} / "
        f"{RED}{unavailable} failed{NC}"
    )

    print()
    print(
        f"{BOLD}>> ATHENAFUSIONX ROLE CONFIGURATION{NC}"
    )
    print(
        f"  • Planner : {CYAN}gemini-3.5-flash-lite{NC}"
        f"  → fallback {CYAN}gemini-2.5-flash-lite{NC}"
    )
    print(
        f"  • Heavy   : {CYAN}z-ai/glm-5.3-flash{NC}"
        f"  → fallback {CYAN}nvidia/ising-calibration-1.5-31b{NC}"
    )
    print(
        f"  • Light   : {CYAN}gemma-4-e4b-it{NC}"
        f"  → Local S23"
    )
    print(
        f"  • Vision  : {CYAN}meta/llama-3.2-11b-vision-instruct{NC}"
    )
    print(
        f"  • Embed   : {CYAN}granite-311m-edge{NC}"
        f"  → 384-dim Duet Edge"
    )

    rec_file = write_recommended_env(
        working_models
    )

    print()
    print(
        f"{GREEN}✔ Generated curated tier settings: "
        f"{rec_file}{NC}"
    )

    print()
    print(
        f"{DIM}No manifest models were added automatically.{NC}"
    )
    print(
        f"{DIM}No API credentials were printed.{NC}"
    )
    print()


if __name__ == "__main__":
    main()
