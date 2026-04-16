#!/bin/bash
BINARY="./zig-out/bin/ziggy-llm"
MODEL="../local_dir/google_gemma-4-E2B-it-Q4_K_M.gguf"

echo "[*] Testing Binary with Minimal Handshake..."
# Test 1: No subcommand, short model flag
$BINARY -m "$MODEL" -p "Status report."

echo "[*] Testing with 'run' instead of 'chat'..."
# Test 2: Some Zig LLM ports use 'run' or 'infer'
$BINARY run --model "$MODEL" --prompt "Hello"
