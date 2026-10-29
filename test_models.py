import os, json, urllib.request

keys = [
    ("PLANNER", "NEXUS_PLANNER_MODEL"),
    ("HEAVY", "NEXUS_HEAVY_MODEL"),
    ("LIGHT", "NEXUS_LIGHT_MODEL"),
]

def get_endpoint(model):
    model_lower = model.lower()
    if "gemini" in model_lower or model_lower.startswith("lyria") or model_lower.startswith("antigravity") or model_lower.startswith("deep-research"):
        return "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions", os.environ.get("GEMINI_API_KEY", "")
    return "https://integrate.api.nvidia.com/v1/chat/completions", os.environ.get("NVIDIA_API_KEY", "")

for name, model_key in keys:
    model = os.environ.get(model_key)
    print("\n" + "="*70)
    print(name)
    print("MODEL:", model)
    
    if not model:
        continue
        
    url, key = get_endpoint(model)
    
    payload = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": "Reply with exactly: ATHENAFUSIONX_MODEL_TEST_OK"}],
        "max_tokens": 50
    }).encode("utf-8")
    
    req = urllib.request.Request(url, data=payload, headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {key}"
    })
    
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            try:
                print("RESPONSE:", data["choices"][0]["message"]["content"].strip())
            except KeyError:
                print("RAW RESPONSE:", json.dumps(data, indent=2))
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8")
        print("HTTP ERROR:", e.code, body)
    except Exception as e:
        print("ERROR:", type(e).__name__, str(e)[:300])
