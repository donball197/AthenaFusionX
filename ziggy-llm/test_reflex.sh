#!/bin/bash
# Project Nexus Singularity - CLI Debug Pulse

BINARY="./zig-out/bin/ziggy-llm"
MODEL="./local_dir/gemma_brain.gguf"

echo "[*] Attempting Barebones Pulse..."
# Trying without the 'chat' subcommand and with short flags
$BINARY -m "$MODEL" -p "Hello" -t 4
