#!/usr/bin/env bash
set -e

ENV_FILE=".env"

show_current() {
    echo -e "\n\x1b[1;36m==================================================\x1b[0m"
    echo -e "\x1b[1;36m>> ATHENAFUSIONX ACTIVE MODEL CONFIGURATION\x1b[0m"
    echo -e "\x1b[1;36m==================================================\x1b[0m"
    grep -E '^(NEXUS_PLANNER_MODEL|NEXUS_LIGHT_MODEL|NEXUS_LIGHT_CASCADE|NEXUS_HEAVY_MODEL|NEXUS_HEAVY_CASCADE)=' "$ENV_FILE" || echo "No models set."
    echo -e "\x1b[1;36m==================================================\x1b[0m\n"
}

set_tier() {
    local tier="$1"
    local model="$2"
    if [ -z "$tier" ] || [ -z "$model" ]; then
        echo -e "\x1b[31mUsage: ./models.sh set [planner|light|heavy|light_cascade|heavy_cascade] <provider/model>\x1b[0m"
        exit 1
    fi

    case "$tier" in
        planner) sed -i "s|^NEXUS_PLANNER_MODEL=.*|NEXUS_PLANNER_MODEL=${model}|" "$ENV_FILE" ;;
        light) sed -i "s|^NEXUS_LIGHT_MODEL=.*|NEXUS_LIGHT_MODEL=${model}|" "$ENV_FILE" ;;
        heavy) sed -i "s|^NEXUS_HEAVY_MODEL=.*|NEXUS_HEAVY_MODEL=${model}|" "$ENV_FILE" ;;
        light_cascade) sed -i "s|^NEXUS_LIGHT_CASCADE=.*|NEXUS_LIGHT_CASCADE=${model}|" "$ENV_FILE" ;;
        heavy_cascade) sed -i "s|^NEXUS_HEAVY_CASCADE=.*|NEXUS_HEAVY_CASCADE=${model}|" "$ENV_FILE" ;;
        *) echo -e "\x1b[31mUnknown tier '$tier'\x1b[0m"; exit 1 ;;
    esac
    echo -e "\x1b[1;32m[+] Updated ${tier} -> ${model}\x1b[0m"
    show_current
}

apply_preset() {
    local preset="$1"
    case "$preset" in
        speed)
            echo -e "\x1b[1;33m>> Applying [SPEED & LOW-LATENCY] Preset...\x1b[0m"
            sed -i "s|^NEXUS_PLANNER_MODEL=.*|NEXUS_PLANNER_MODEL=google/gemini-3.6-flash|" "$ENV_FILE"
            sed -i "s|^NEXUS_LIGHT_MODEL=.*|NEXUS_LIGHT_MODEL=google/gemini-3.6-flash|" "$ENV_FILE"
            sed -i "s|^NEXUS_LIGHT_CASCADE=.*|NEXUS_LIGHT_CASCADE=nvidia/ising-calibration-1.5-31b|" "$ENV_FILE"
            sed -i "s|^NEXUS_HEAVY_MODEL=.*|NEXUS_HEAVY_MODEL=google/gemini-3.6-flash|" "$ENV_FILE"
            sed -i "s|^NEXUS_HEAVY_CASCADE=.*|NEXUS_HEAVY_CASCADE=nvidia/deepseek-ai/deepseek-v4-flash-0731|" "$ENV_FILE"
            ;;
        surgery|deep)
            echo -e "\x1b[1;33m>> Applying [DEEP CODE SURGERY & HIGH-ACCURACY] Preset...\x1b[0m"
            sed -i "s|^NEXUS_PLANNER_MODEL=.*|NEXUS_PLANNER_MODEL=nvidia/deepseek-ai/deepseek-v4-flash-0731|" "$ENV_FILE"
            sed -i "s|^NEXUS_LIGHT_MODEL=.*|NEXUS_LIGHT_MODEL=nvidia/ising-calibration-1.5-31b|" "$ENV_FILE"
            sed -i "s|^NEXUS_LIGHT_CASCADE=.*|NEXUS_LIGHT_CASCADE=google/gemini-3.6-flash|" "$ENV_FILE"
            sed -i "s|^NEXUS_HEAVY_MODEL=.*|NEXUS_HEAVY_MODEL=nvidia/deepseek-ai/deepseek-v4-flash-0731|" "$ENV_FILE"
            sed -i "s|^NEXUS_HEAVY_CASCADE=.*|NEXUS_HEAVY_CASCADE=google/gemini-3.6-flash|" "$ENV_FILE"
            ;;
        bob)
            echo -e "\x1b[1;33m>> Applying [BOB AGENT HYBRID MESH] Preset...\x1b[0m"
            sed -i "s|^NEXUS_PLANNER_MODEL=.*|NEXUS_PLANNER_MODEL=google/gemini-3.6-flash|" "$ENV_FILE"
            sed -i "s|^NEXUS_LIGHT_MODEL=.*|NEXUS_LIGHT_MODEL=bob/agent-v1|" "$ENV_FILE"
            sed -i "s|^NEXUS_LIGHT_CASCADE=.*|NEXUS_LIGHT_CASCADE=nvidia/ising-calibration-1.5-31b,google/gemini-3.6-flash|" "$ENV_FILE"
            sed -i "s|^NEXUS_HEAVY_MODEL=.*|NEXUS_HEAVY_MODEL=bob/agent-v1|" "$ENV_FILE"
            sed -i "s|^NEXUS_HEAVY_CASCADE=.*|NEXUS_HEAVY_CASCADE=nvidia/deepseek-ai/deepseek-v4-flash-0731|" "$ENV_FILE"
            ;;
        *)
            echo -e "\x1b[31mUnknown preset. Available presets: speed | deep | bob\x1b[0m"
            exit 1
            ;;
    esac
    show_current
}

case "$1" in
    set) set_tier "$2" "$3" ;;
    preset|profile|apply) apply_preset "$2" ;;
    list|show|"") show_current ;;
    *) echo "Usage: ./models.sh [show | set <tier> <model> | preset <speed|deep|bob>]" ;;
esac
