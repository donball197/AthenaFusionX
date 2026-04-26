#!/usr/bin/env bash
# scout_models.sh — Nexus Quota-Aware Model Scout (v3 - Fixed)

[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo -e "\e[31m>> [ERROR] Run from project root\e[0m" && exit 1
set -e

if [ -f .env ]; then 
    export $(grep -v '^#' .env | xargs)
fi

if [ -z "$GEMINI_API_KEY" ]; then
    echo -e "\e[31m>> [ERROR] GEMINI_API_KEY missing in .env\e[0m"
    exit 1
fi

echo -e "\e[36m>> [NEXUS] Fetching models + limits...\e[0m"

curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY" | python3 -c '
import sys, json

try:
    data = json.load(sys.stdin)
except:
    print("\033[31m>> [FATAL] Invalid API response. Check key/network.\033[0m")
    sys.exit(1)

if "error" in data:
    print("\033[31m>> [API ERROR] " + data["error"].get("message", "Unknown") + "\033[0m")
    sys.exit(1)

models = data.get("models", [])
models.sort(key=lambda x: int(x.get("inputTokenLimit", 0)), reverse=True)

print("\n\033[1;36mMODEL NAME" + " " * 35 + " | INPUT     | OUTPUT    | RECOMMENDED TIER\033[0m")
print("\033[90m" + "-" * 85 + "\033[0m")

for m in models:
    name = m.get("name", "").replace("models/", "")
    input_lim = str(m.get("inputTokenLimit", "N/A"))
    output_lim = str(m.get("outputTokenLimit", "N/A"))
    
    if "gemma-4-26b" in name:
        tier = "PRO (Safe daily driver)"
        color = "\033[32m"
    elif "gemma-4-31b" in name:
        tier = "ELITE (High headroom)"
        color = "\033[33m"
    elif "2.5-flash" in name.lower() or "flash" in name.lower():
        tier = "STD (Use sparingly - tight RPD)"
        color = "\033[35m"
    else:
        tier = "Other"
        color = "\033[90m"
    
    print(color + name.ljust(45) + "\033[0m | \033[33m" + input_lim.ljust(9) + "\033[0m | \033[33m" + output_lim.ljust(9) + "\033[0m | " + tier)
'

echo -e "\n\e[33m>> [WARNING] Gemini 2.5 Flash has very low daily requests (~20 RPD). Prefer Gemma 4 26B for most work.\e[0m"
echo -e "\e[36m   Check exact quotas at: https://aistudio.google.com/rate-limit\e[0m"
