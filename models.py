import os
import json
import urllib.request
import urllib.error
import concurrent.futures

# ANSI Colors
CYAN, GREEN, YELLOW, PURPLE, RED, BLUE, RESET, BOLD = (
    '\033[36m', '\033[32m', '\033[33m', '\033[35m', '\033[31m', '\033[34m', '\033[0m', '\033[1m'
)
NVIDIA_GREEN = '\033[92m'

TIMEOUT_SECS = 5

def format_tokens(num):
    if not isinstance(num, (int, float)): return "N/A"
    if num >= 1_000_000:
        return f"{num / 1_000_000:.1f}M"
    elif num >= 1_000:
        return f"{num / 1_000:.1f}K"
    return str(num)

def load_env():
    if os.path.exists(".env"):
        with open(".env") as f:
            for line in f:
                if line.strip() and not line.startswith("#"):
                    key, _, value = line.partition("=")
                    os.environ[key.strip()] = value.strip().strip("'\"")

def fetch_api(url, headers=None, parser_func=None, provider_name="API"):
    headers = headers or {}
    try:
        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=TIMEOUT_SECS) as response:
            data = json.loads(response.read().decode())
            return parser_func(data)
    except urllib.error.HTTPError as e:
        print(f"{RED}[ERROR] {provider_name} returned HTTP {e.code}: {e.reason}{RESET}")
    except urllib.error.URLError as e:
        print(f"{RED}[ERROR] {provider_name} network fault: {e.reason}{RESET}")
    except Exception as e:
        print(f"{RED}[ERROR] {provider_name} unexpected failure: {e}{RESET}")
    return []

def parse_google(data):
    results = []
    for m in data.get("models", []):
        if "generateContent" in m.get("supportedGenerationMethods", []):
            name = m.get("name", "").replace("models/", "")
            cat = "GEMMA" if "gemma" in name else "FLASH" if "flash" in name else "PRO"
            color = PURPLE if "gemma" in name else BLUE if "flash" in name else YELLOW
            results.append({
                "provider": "Google", "name": name,
                "context": format_tokens(m.get("inputTokenLimit", 0)),
                "category": cat, "color": color
            })
    return results

def parse_openai(data):
    results = []
    for m in data.get("data", []):
        name = m.get("id", "")
        if "gpt" in name or "o1" in name or "o3" in name:
            cat = "REASONING" if "o1" in name or "o3" in name else "GPT"
            results.append({
                "provider": "OpenAI", "name": name,
                "context": "Dynamic", "category": cat, "color": CYAN
            })
    return results

def parse_anthropic(data):
    results = []
    for m in data.get("data", []):
        name = m.get("id", "")
        cat = "OPUS" if "opus" in name else "SONNET" if "sonnet" in name else "HAIKU"
        results.append({
            "provider": "Anthropic", "name": name,
            "context": "Dynamic", "category": cat, "color": YELLOW
        })
    return results

def parse_nvidia(data):
    family_map = {
        "llama": ("LLAMA", YELLOW),
        "mistral": ("MISTRAL", BLUE),
        "nemotron": ("NEMOTRON", PURPLE),
        "mixtral": ("MISTRAL", BLUE),
        "gemma": ("GEMMA", PURPLE)
    }
    results = []
    for m in data.get("data", []):
        name = m.get("id", "")
        n_lower = name.lower()
        cat, color = ("NVIDIA", NVIDIA_GREEN)
        for key, (mapped_cat, mapped_color) in family_map.items():
            if key in n_lower:
                cat, color = mapped_cat, mapped_color
                break
        results.append({
            "provider": "Nvidia", "name": name,
            "context": "Dynamic", "category": cat, "color": color
        })
    return results

def main():
    load_env()
    print(f"{BOLD}{CYAN}>> INITIALIZING UNIFIED CORTEX MANIFEST [NEXUS v3.3 - Parallel]{RESET}")
    print("-" * 85)

    tasks = []
    
    if g_key := os.environ.get("GEMINI_API_KEY"):
        print(f"{BLUE}>> Dispatching Google AI Studio probe...{RESET}")
        url = f"https://generativelanguage.googleapis.com/v1beta/models?key={g_key}"
        tasks.append((url, {}, parse_google, "Google"))
    else:
        print(f"{RED}>> Skipped Google (No API Key){RESET}")

    if n_key := os.environ.get("NVIDIA_API_KEY"):
        print(f"{NVIDIA_GREEN}>> Dispatching NVIDIA NIM probe...{RESET}")
        url = "https://integrate.api.nvidia.com/v1/models"
        tasks.append((url, {"Authorization": f"Bearer {n_key}"}, parse_nvidia, "NVIDIA"))
    else:
        print(f"{YELLOW}>> Skipped NVIDIA (No API Key){RESET}")

    if o_key := os.environ.get("OPENAI_API_KEY"):
        print(f"{CYAN}>> Dispatching OpenAI Platform probe...{RESET}")
        url = "https://api.openai.com/v1/models"
        tasks.append((url, {"Authorization": f"Bearer {o_key}"}, parse_openai, "OpenAI"))
    else:
        print(f"{YELLOW}>> Skipped OpenAI (No API Key){RESET}")

    if a_key := os.environ.get("ANTHROPIC_API_KEY"):
        print(f"{YELLOW}>> Dispatching Anthropic Console probe...{RESET}")
        url = "https://api.anthropic.com/v1/models"
        tasks.append((url, {"x-api-key": a_key, "anthropic-version": "2023-06-01"}, parse_anthropic, "Anthropic"))
    else:
        print(f"{YELLOW}>> Skipped Anthropic (No API Key){RESET}")

    all_models = []
    
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        futures = [
            executor.submit(fetch_api, url, headers, parser, name) 
            for url, headers, parser, name in tasks
        ]
        for future in concurrent.futures.as_completed(futures):
            all_models.extend(future.result())

    print("\n" + "-" * 85)
    print(f"{BOLD}{'PROVIDER':<12} | {'MODEL ID':<45} | {'CONTEXT':<8} | {'CATEGORY':<15}{RESET}")
    print("-" * 85)

    for m in sorted(all_models, key=lambda x: (x['provider'], x['name'])):
        print(f"{m['color']}{m['provider']:<12}{RESET} | {m['color']}{m['name']:<45}{RESET} | {m['context']:<8} | {m['category']:<15}")

    print(f"\n{BOLD}{GREEN}✅ Unified manifest generated in parallel!{RESET}")
    
    with open(".nexus_manifest.json", "w") as f:
        json.dump(all_models, f, indent=2)

if __name__ == "__main__":
    main()
