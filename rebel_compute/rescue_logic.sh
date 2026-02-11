# 1. Search the Debian root and the Motorola storage
FOUND_PATH=$(find /data/data/com.termux/files -name "nexus_logic_v12.bin" | head -n 1)

if [ -z "$FOUND_PATH" ]; then
    echo "[!] Logic file is GONE. Agents are mute."
else
    echo "[+] Found at: $FOUND_PATH"
    mkdir -p legacy_work
    cp "$FOUND_PATH" ./legacy_work/nexus_logic_v12.bin
    echo "[+] 12th Layer Anchored. Ready for Deep Scan."
fi
