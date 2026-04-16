#!/bin/bash
# Project Nexus Singularity - v1.9 Corrected Reflex

BINARY="./zig-out/bin/ziggy-llm"
# Path seated in the parent dev directory
MODEL="../local_dir/google_gemma-4-E2B-it-Q4_K_M.gguf"

if [ ! -f "$BINARY" ]; then
    echo "[*] Compiling Native Muscles..."
    zig build -Doptimize=ReleaseFast
fi

echo "[*] Triggering Gemma 4 2b Reflex..."
# ORDER MATTERS: <command> must come first.
# No --threads flag (not in your cli.zig).
$BINARY chat \
  --model "$MODEL" \
  --backend cpu \
  --temperature 0.7 \
  --moon-quant enabled
