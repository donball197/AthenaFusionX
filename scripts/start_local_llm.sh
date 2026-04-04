#!/usr/bin/env bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
set -e
cd "$(dirname "$0")/.."

LLAMA_SERVER="vendor/llama_build/bin/llama-server"
SMOLLM="vault/models/smollm2-256m.gguf"
GEMMA_1B="$HOME/nexus_hybrid/vault/gemma1b.gguf"

pkill -f "llama-server" 2>/dev/null || true
sleep 1

MODE="${1:-micro}"

case "$MODE" in
    micro) MODEL="$SMOLLM"; CTX=2048; echo ">> [LLM] MICRO mode — SmolLM2 256M (~180MB RAM)" ;;
    fast)  MODEL="$GEMMA_1B"; CTX=4096; echo ">> [LLM] FAST mode — Gemma 1B (~800MB RAM)" ;;
    *)     echo "Usage: $0 [micro|fast]"; exit 1 ;;
esac

if [ ! -f "$MODEL" ]; then
    echo ">> [LLM] Model not found: $MODEL — Gemini fallback active."
    exit 0
fi

"$LLAMA_SERVER" \
    --model "$MODEL" \
    --host 127.0.0.1 \
    --port 8080 \
    --ctx-size "$CTX" \
    --n-predict 1024 \
    --threads 2 \
    --batch-size 64 \
    --mlock \
    --no-mmap \
    --log-disable &

echo ">> [LLM] PID: $! — Waiting for ready..."
for i in $(seq 1 40); do
    curl -sf http://127.0.0.1:8080/health >/dev/null 2>&1 && echo ">> [LLM] Ready." && exit 0
    sleep 1
done
echo ">> [LLM] Timeout — Gemini fallback active."
