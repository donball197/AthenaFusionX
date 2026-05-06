#!/bin/bash
echo "=== Preparing Lenovo Duet for Rust Compilation (4GB RAM) ==="

# Create swap if not exists
if [ ! -f /swapfile ]; then
    echo "Creating 6GB swapfile..."
    sudo fallocate -l 6G /swapfile
    sudo chmod 600 /swapfile
    sudo mkswap /swapfile
    sudo swapon /swapfile
    echo "/swapfile none swap sw 0 0" | sudo tee -a /etc/fstab
    echo "Swap created and enabled."
else
    echo "Swapfile already exists."
fi

free -h

echo -e "\nRecommended before building:"
echo "   • Close all Chrome tabs"
echo "   • Run: watch -n 2 free -h   (in another terminal)"
echo "   • Use: ./build.sh"
