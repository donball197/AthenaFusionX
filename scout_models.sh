#!/usr/bin/env bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
set -e

# Securely load the vault
if [ -f .env ]; then 
    export $(grep -v '^#' .env | xargs)
fi

if [ -z "$GEMINI_API_KEY" ]; then
    echo -e "\e[31m>> [ERROR] GEMINI_API_KEY not found in .env vault.\e[0m"
    exit 1
fi

echo -e "\e[36m>> [NEXUS] Pinging Generative Language API...\e[0m"

curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY" | \
python3 -c "
import sys, json

try:
    data = json.load(sys.stdin)
except json.JSONDecodeError:
    print('\033[31m>> [FATAL] Invalid response from API. Verify network link and vault credentials.\033[0m')
    sys.exit(1)

if 'error' in data:
    print(f'\033[31m>> [API REFUSAL] {data[\"error\"].get(\"message\", \"Unknown anomaly\")}\033[0m')
    sys.exit(1)

models = data.get('models', [])

# Sort architecture by Input Limit (Descending) to prioritize heavy lifters
models.sort(key=lambda x: int(x.get('inputTokenLimit', 0)), reverse=True)

print(f'\n\033[1;36m{\"MODEL NAME\":<40} | {\"INPUT LIMIT\":<12} | {\"OUTPUT LIMIT\"}\033[0m')
print('\033[90m' + '-'*75 + '\033[0m')

for m in models:
    name = m.get('name', '').replace('models/', '')
    itask = m.get('inputTokenLimit', 'N/A')
    otask = m.get('outputTokenLimit', 'N/A')
    
    # Print with Nexus terminal aesthetics
    print(f'\033[32m{name:<40}\033[0m | \033[33m{itask:<12}\033[0m | \033[33m{otask}\033[0m')
"
