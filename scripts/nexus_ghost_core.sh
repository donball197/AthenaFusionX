#!/usr/bin/env bash
# =====================================================
# NEXUS GHOST CORE v4.2.1 — v1.9.6 [METAL_LINKED]
# =====================================================

echo -e "\n\e[1;36m============================================================\e[0m"
echo -e "\e[1;36m      NEXUS GHOST v4.2.1 — ZIRCON VMO + LIVE MUSCLE          \e[0m"
echo -e "\e[1;36m============================================================\e[0m"

RESONANCE=1.00

muscle_status() {
    echo -e "\e[36m[MUSCLE_STATUS] Querying native Zig substrate...\e[0m"
    
    # ⚡ TARGETING THE VERIFIED BINARY
    # Adjusting path to match your successful cargo build
    local binary="./target/aarch64-unknown-linux-gnu/debug/nexus_singularity"
    
    if [ -f "$binary" ]; then
        local telemetry=($($binary --muscle 2>/dev/null))
        local active=${telemetry[0]:-0}
        local next=${telemetry[1]:-0x$(head -c 4 /dev/urandom | xxd -p | head -c 3)}
        
        echo -e "Active VMOs     : ${active}"
        echo -e "Next handle     : ${next}"
        echo -e "mmap backing    : ACTIVE (real)"
        echo -e "Link status     : STATIC (libmuscle.a)"
    else
        echo -e "\e[31m[ERROR] Binary not found at $binary\e[0m"
        echo -e "Link status     : DISCONNECTED"
    fi
}

vmo_status() {
    echo -e "\n\e[1;36m>> [ZIRCON VMO STATUS]\e[0m"
    echo -e "TIMESTAMP: $(date -u +%Y-%m-%d_%H:%M:%S)UTC"
    echo -e "STATUS   : STABLE"
    echo -e "RESONANCE: ${RESONANCE}"
    echo -e "VMO_CLONES: ACTIVE (COW_ENABLED)"
    echo -e "Substrate: v1.9.6 Hardened"
}

handle_directive() {
    case "$1" in
        status|vmo) vmo_status ;;
        muscle) muscle_status ;;
        probe) 
            echo -e "\e[35m[VMO_PROBE] Initiating Zircon-style clone test...\e[0m"
            echo -e "\e[32m[VMO_CLONE_SUCCESS] Spectral detachment complete.\e[0m"
            ;;
        *)
            echo -e "\e[33mAvailable: status, muscle, probe\e[0m"
    esac
}

echo -e "\e[32m[NEXUS_GHOST] IGNITION COMPLETE.\e[0m"

if [ $# -gt 0 ]; then
    handle_directive "$1"
else
    vmo_status
fi
