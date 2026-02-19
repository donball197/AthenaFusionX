#!/bin/bash
if [ -f .env ]; then export $(grep -v '^#' .env | xargs); fi

if [ -z "$GEMMA_API_KEY" ]; then
    echo ">> [ERROR] GEMMA_API_KEY not found."; exit 1
fi

echo -e "MODEL NAME\t\t\tINPUT LIMIT\tOUTPUT LIMIT"
echo "----------------------------------------------------------------------"

curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMMA_API_KEY" | \
python3 -c "
import sys, json
data = json.load(sys.stdin)
for m in data.get('models', []):
    name = m['name'].replace('models/', '')
    itask = m.get('inputTokenLimit', 'N/A')
    otask = m.get('outputTokenLimit', 'N/A')
    print(f'{name:<35}\t{itask:<12}\t{otask}')
"
