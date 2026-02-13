#!/bin/bash
# The Oracle: Queries Google for the current active model list

# Ensure API Key is present
if [ -z "$GEMINI_API_KEY" ]; then
    echo ">> [ERROR] GEMINI_API_KEY not found in environment."
    exit 1
fi

echo ">> [NEXUS] Querying API for available models..."

# Query the v1beta models endpoint
curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=${GEMINI_API_KEY}" \
    | jq -r '.models[] | "\(.name) \t [\(.displayName)]"' \
    | grep -E "gemma|gemini"
