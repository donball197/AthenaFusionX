#!/bin/bash
# Project Nexus Singularity - v1.9 Weight Acquisition (Hardened)

VENV_PATH="../.nexus_venv"

if [ -d "$VENV_PATH" ]; then
    echo "[*] Activating parent .nexus_venv..."
    source "$VENV_PATH/bin/activate"
else
    echo "[!] venv missing. Manifesting .nexus_venv..."
    python3 -m venv "$VENV_PATH"
    source "$VENV_PATH/bin/activate"
fi

echo "[*] Ensuring HuggingFace Muscles are seated..."
pip install --upgrade pip
pip install huggingface-hub

echo "[*] Initializing Download: Gemma 4 2b (E2B) Q4_K_M..."
mkdir -p ../local_dir

# Using the direct binary seated in the venv's bin
huggingface-cli download bartowski/google_gemma-4-E2B-it-GGUF \
  google_gemma-4-E2B-it-Q4_K_M.gguf \
  --local-dir ../local_dir \
  --local-dir-use-symlinks False

echo "[+] Weight seated at ../local_dir/google_gemma-4-E2B-it-Q4_K_M.gguf"
deactivate
