#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
# NEXUS Cost-Control Tool — Focused on gemma-3-27b-it

API_KEY="${GEMINI_API_KEY}"
if [ -z "$API_KEY" ]; then
    echo "Error: GEMINI_API_KEY not set. Run: export GEMINI_API_KEY=your_key"
    exit 1
fi

PROMPT=""$1""
MODEL="${2:-gemma-3-27b-it}"

if [ -z "$PROMPT" ]; then
    PROMPT="Improve thermal throttling logic in the orchestrator for MT8183 ARM64 device"
fi

echo ">> [NEXUS] Token Counter + Model Explorer"
echo "Model in use : $MODEL"
echo "Prompt       : ${PROMPT:0:90}..."
echo "--------------------------------------------------"

# Count tokens
TOKEN_RESPONSE=$(curl -s -X POST \
  "https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:countTokens?key=$API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "contents": [{
      "role": "user",
      "parts": [{"text": "'"$PROMPT"'"}]
    }]
  }')

TOTAL_TOKENS=$(echo "$TOKEN_RESPONSE" | jq -r '.totalTokens // 0')

echo "Total Tokens : $TOTAL_TOKENS"

# Very rough cost estimate (Gemma is significantly cheaper)
if [[ $MODEL == *"gemma"* ]]; then
    EST_COST=$(awk "BEGIN {printf \"%.7f\", $TOTAL_TOKENS * 0.00000035}")
    echo "Est. Cost    : ~ \$$EST_COST USD (Gemma)"
else
    EST_COST=$(awk "BEGIN {printf \"%.7f\", $TOTAL_TOKENS * 0.0000035}")
    echo "Est. Cost    : ~ \$$EST_COST USD"
fi

echo ""
echo "Quick usage:"
echo "  ./find_models_and_tokens.sh \"Your prompt here\""
echo "  ./find_models_and_tokens.sh \"Your prompt here\" gemini-2.5-flash"
