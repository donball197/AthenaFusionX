#!/bin/bash
# Project Nexus Singularity v1.9 - Modern Brain Seat (hf-cli)

echo "[*] Activating nexus_venv..."
# The venv is already there, we just need to ensure 'hf' is accessible
source .nexus_venv/bin/activate

echo "[*] Seating Gemma 4 2b (E2B) Q4_K_M via 'hf'..."
mkdir -p local_dir

# Modern 'hf' syntax for targeted GGUF download
hf download bartowski/google_gemma-4-E2B-it-GGUF \
  google_gemma-4-E2B-it-Q4_K_M.gguf \
  --local-dir ./local_dir

echo "[+] Verify Muscle Seat:"
ls -lh ./local_dir/google_gemma-4-E2B-it-Q4_K_M.gguf
deactivate
