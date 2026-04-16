#!/bin/bash
echo ">> [NEXUS] Initializing Mobile DNA Synthesis..."

# 1. Install the Android Command Line Tools (No IDE)
# Note: You need the JDK for Android
sudo apt update
sudo apt install openjdk-17-jdk-headless -y

# 2. Install Tauri CLI (The Bridge)
cargo install tauri-cli --version 2.0.0-beta

# 3. Add Android Rust Targets
rustup target add aarch64-linux-android armv7-linux-androideabi

echo ">> [SUCCESS] Mobile Substrate ready. You can now build .apk files."
echo ">> [TIP] Use a physical Android phone via USB. DO NOT run an emulator on 4GB RAM."
