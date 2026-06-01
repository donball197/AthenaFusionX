#!/bin/bash

# 1. Setup Swap (Crucial for Chromebooks)
if [ ! -f /swapfile ]; then
    echo "Creating 4GB swapfile to prevent OOM kills..."
    sudo fallocate -l 4G /swapfile
    sudo chmod 600 /swapfile
    sudo mkswap /swapfile
    sudo swapon /swapfile
    echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
fi

# 2. Set Cargo environment variables
# Reduces memory pressure by limiting parallel codegen units
echo 'export CARGO_BUILD_JOBS=2' >> ~/.bashrc
export CARGO_BUILD_JOBS=2

# 3. Suggesting Mold linker if not present
if ! command -v mold &> /dev/null; then
    echo "Consider installing mold for faster linking: sudo apt install mold"
fi

# 4. Cleanup/Sync
source ~/.bashrc
echo "Optimizations applied. Please use 'cargo build --jobs 2' for builds."
