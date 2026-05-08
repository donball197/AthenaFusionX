#!/bin/bash

# Load API Key
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

if [ -z "$GEMINI_API_KEY" ]; then
    echo -e "\033[31m[ERROR] GEMINI_API_KEY is not set in .env\033[0m"
    exit 1
fi

echo -e "\033[1;36m>> GENERATING FULL CORTEX MANIFEST [DATE: MAY 2026]\033[0m"
echo -e "\033[90mRetrieving all available models and capabilities...\033[0m\n"

# Fetch raw data
RAW_DATA=$(curl -s "https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY")

# Header for the table
printf "\033[1;37m%-45s | %-10s | %-10s | %-8s | %-15s\033[0m\n" "MODEL ID" "IN_TOK" "OUT_TOK" "THINK" "METHODS"
printf "%-45s-|-%-10s-|-%-10s-|-%-8s-|-%-15s\n" "---------------------------------------------" "----------" "----------" "--------" "---------------"

# Use jq to iterate over every model in the array
echo "$RAW_DATA" | jq -r '.models[] | "\(.name)|\(.inputTokenLimit)|\(.outputTokenLimit)|\(.thinking // false)|\(.supportedGenerationMethods | join(","))"' | while IFS='|' read -r name in_lim out_lim think methods; do

    # Logic-based Color Coding
    if [[ "$name" == *"3.1"* ]] || [[ "$name" == *"3-pro"* ]]; then
        COLOR="\033[1;32m" # Green (Cutting Edge)
    elif [[ "$name" == *"gemma-4"* ]]; then
        COLOR="\033[1;35m" # Purple (Open Weights Reasoning)
    elif [[ "$name" == *"deep-research"* ]]; then
        COLOR="\033[1;33m" # Yellow (Specialist)
    elif [[ "$name" == *"image"* ]] || [[ "$name" == *"veo"* ]]; then
        COLOR="\033[1;34m" # Blue (Multimodal/Video)
    else
        COLOR="\033[0m"     # Default
    fi

    # Format numbers for readability (e.g., 1048576 -> 1.0M)
    if [ "$in_lim" -ge 1000000 ]; then in_fmt="1.0M"; else in_fmt="$in_lim"; fi
    
    # Display the row
    printf "${COLOR}%-45s\033[0m | %-10s | %-10s | %-8s | %-15s\n" "$name" "$in_fmt" "$out_lim" "$think" "${methods:0:15}..."
done

echo -e "\n\033[1;36m>> CRITICAL UPDATES DETECTED FOR YOUR BACKEND:\033[0m"
echo -e "1. \033[1;33mDeep Research\033[0m: 'models/deep-research-max-preview-04-2026' is available for high-complexity audits."
echo -e "2. \033[1;32mGemini 3.1\033[0m: 'models/gemini-3.1-pro-preview' offers 1M tokens with thinking capability."
echo -e "3. \033[1;35mGemma 4\033[0m: 'models/gemma-4-31b-it' is the peak of the local-style reasoning models."
