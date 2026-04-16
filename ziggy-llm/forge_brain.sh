#!/bin/bash
# Project Nexus Singularity - v1.9 Sovereign Brain Forge

RAW_URL="https://huggingface.co/bartowski/google_gemma-4-E2B-it-GGUF/resolve/main/google_gemma-4-E2B-it-Q4_K_M.gguf"
LOCAL_DIR="../local_dir"
PURE_MODEL="$LOCAL_DIR/pure_gemma4_2b.gguf"

echo "[*] Initializing the Forge..."
mkdir -p "$LOCAL_DIR"

echo "[*] Bypassing Python. Pulling pristine GGUF via raw uplink..."
# wget is native, resilient, and doesn't care about pip modules
wget -O "$PURE_MODEL" "$RAW_URL" --show-progress

echo "[+] Raw Material Acquired: $PURE_MODEL"

echo "[*] Auditing Pure DNA with Ziggy-LLM..."
./zig-out/bin/ziggy-llm inspect -m "$PURE_MODEL"
