#!/usr/bin/env python3

import os
import json
import urllib.request
import urllib.error
import concurrent.futures

VERSION = "4.0.2"
TIMEOUT_SECS = 8
JSON_OUTPUT = ".nexus_manifest.json"

RESET = "\033[0m"
BOLD = "\033[1m"
CYAN = "\033[36m"
GREEN = "\033[32m"
YELLOW = "\033[33m"
PURPLE = "\033[35m"
RED = "\033[31m"
BLUE = "\033[34m"
NVIDIA_GREEN = "\033[92m"

def load_env():
    if not os.path.exists(".env"): return
    with open(".env", "r", encoding="utf-8") as f:
        for raw_line in f:
            line = raw_line.strip()
            if not line or line.startswith("#"): continue
            key, sep, value = line.partition("=")
            if sep and key.strip() not in os.environ:
                os.environ[key.strip()] = value.strip().strip("'\"")

def format_tokens(value):
    if not isinstance(value, (int, float)) or value <= 0: return "Unknown"
    if value >= 1_000_000: return f"{value / 1_000_000:.1f}M"
    if value >= 1_000: return f"{value / 1_000:.1f}K"
    return str(int(value))

def classify_model(name):
    n = name.lower()
    if "embed" in n: return "EMBEDDING"
    if "safety" in n or "guard" in n: return "SAFETY"
    if "vision" in n or "-vl" in n: return "VISION"
    if "gemma" in n: return "GEMMA"
    if "llama" in n: return "LLAMA"
    if "mistral" in n: return "MISTRAL"
    if "nemotron" in n: return "NEMOTRON"
    if "flash" in n: return "FLASH"
    if "pro" in n: return "PRO"
    return "GENERAL"

def color_for_category(category):
    return {"GEMMA": PURPLE, "FLASH": BLUE, "PRO": YELLOW, "LLAMA": YELLOW, "MISTRAL": BLUE, "NEMOTRON": PURPLE}.get(category, NVIDIA_GREEN)

def fetch_api(url, headers=None, parser_func=None, provider_name="API"):
    headers = headers or {}
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "AthenaFusionX/4.0", **headers})
        with urllib.request.urlopen(req, timeout=TIMEOUT_SECS) as response:
            raw = response.read().decode("utf-8")
            return parser_func(json.loads(raw)) if raw else []
    except Exception as e:
        print(f"{RED}[ERROR] {provider_name} failure: {e}{RESET}")
    return []

def parse_google(data):
    results = []
    for model in data.get("models", []):
        if "generateContent" not in model.get("supportedGenerationMethods", []): continue
        name = model.get("name", "").replace("models/", "")
        results.append({
            "provider": "Google", "name": name,
            "context": f"{format_tokens(model.get('inputTokenLimit'))} in / {format_tokens(model.get('outputTokenLimit'))} out",
            "category": classify_model(name)
        })
    return results

def parse_nvidia(data):
    family_map = {
        "llama-3.1": "128K in / 4K out",
        "llama-3.2": "128K in / 4K out",
        "llama-3.3": "128K in / 4K out",
        "llama": "8K in / 4K out",
        "mistral-large": "128K in / 4K out",
        "mistral": "32K in / 4K out",
        "nemotron-4": "128K in / 4K out",
        "nemotron": "4K in / 4K out",
        "mixtral": "32K in / 4K out",
        "gemma-2": "8K in / 4K out",
        "gemma": "8K in / 4K out",
        "deepseek": "128K in / 4K out",
        "phi-3": "128K in / 4K out",
        "granite": "128K in / 4K out",
        "glm": "128K in / 4K out",
        "yi": "32K in / 4K out",
        "jamba": "256K in / 4K out",
        "starcoder": "16K in / 4K out",
        "palmyra": "32K in / 4K out",
        "embed": "8K in / N/A"
    }
    results = []
    for model in data.get("data", []):
        name = model.get("id", "")
        n_lower = name.lower()
        ctx = "Unknown"
        for key, mapped_ctx in family_map.items():
            if key in n_lower:
                ctx = mapped_ctx
                break
        results.append({
            "provider": "Nvidia", "name": name,
            "context": ctx, "category": classify_model(name)
        })
    return results

def main():
    load_env()
    print(f"{BOLD}{CYAN}>> INITIALIZING ATHENAFUSIONX MODEL MANIFEST [NEXUS v{VERSION}]{RESET}")
    print("-" * 110)

    tasks = []

    if g_key := os.environ.get("GEMINI_API_KEY"):
        google_url = "https://" + "generativelanguage.googleapis.com/v1beta/models?key=" + g_key
        tasks.append((google_url, {}, parse_google, "Google"))

    if n_key := os.environ.get("NVIDIA_API_KEY"):
        nvidia_url = "https://" + "integrate.api.nvidia.com/v1/models"
        tasks.append((nvidia_url, {"Authorization": f"Bearer {n_key}"}, parse_nvidia, "NVIDIA"))

    all_models = []
    if tasks:
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
            futures = [executor.submit(fetch_api, u, h, p, n) for u, h, p, n in tasks]
            for future in concurrent.futures.as_completed(futures):
                try: all_models.extend(future.result())
                except Exception as e: pass

    all_models.sort(key=lambda x: (x.get("provider", ""), x.get("name", "")))

    print(f"{BOLD}{'PROVIDER':<12} | {'MODEL ID':<48} | {'TOKENS':<22} | {'CATEGORY':<14}{RESET}")
    print("-" * 110)

    for m in all_models:
        cat = m.get("category", "GENERAL")
        color = color_for_category(cat)
        print(f"{color}{m.get('provider', ''):<12}{RESET} | {color}{m.get('name', ''):<48}{RESET} | {m.get('context', 'Unknown'):<22} | {cat:<14}")

    with open(JSON_OUTPUT, "w") as f: json.dump(all_models, f, indent=2)
    print(f"\n{BOLD}{GREEN}✅ Manifest generated successfully{RESET}")

if __name__ == "__main__":
    main()
