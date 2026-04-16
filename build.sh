#!/bin/bash
echo "=== Nexus Singularity - Optimized Build for Lenovo Duet 4GB ==="

free -h

echo -e "\nSelect build type:"
echo "1) Dev Build"
echo "2) Release Build"
echo "3) Smallest Release (release-small - lowest RAM usage)"
echo "4) Clean + Release"
read -p "Choice (1-4): " choice

case $choice in
    1)
        echo "→ Building Dev..."
        cargo build -j 2
        ;;
    2)
        echo "→ Building Release..."
        cargo build --release -j 2
        ;;
    3)
        echo "→ Building Smallest Release..."
        cargo build --release --profile release-small -j 1
        ;;
    4)
        echo "→ Cleaning and building Release..."
        cargo clean
        cargo build --release -j 2
        ;;
    *)
        echo "Invalid option"
        exit 1
        ;;
esac

echo -e "\n✅ Build finished!"
ls -lh target/aarch64-unknown-linux-gnu/release/nexus_singularity 2>/dev/null || true
free -h
