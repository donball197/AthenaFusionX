#!/bin/bash
# Nexus Singularity: Synchronization Audit Script

echo -e "\e[1;36m=== NEXUS_OS SYNCHRONIZATION AUDIT ===\e[0m"

# 1. Audit the Bridge (Vite Proxy Config)
PROXY_TARGET=$(grep -o 'http://localhost:3000' bins/ide_backend/ui/vite.config.js)
if [ "$PROXY_TARGET" == "http://localhost:3000" ]; then
    echo -e "\e[32m[PASS]\e[0m Bridge: Vite proxy configured to port 3000."
else
    echo -e "\e[31m[FAIL]\e[0m Bridge: Vite proxy mismatch. Expected port 3000."
fi

# 2. Audit the Layout (Checking for Rail implementation status)
if [ -f "bins/ide_backend/ui/src/lib/components/RightSideRail.svelte" ]; then
    echo -e "\e[32m[PASS]\e[0m Rail: RightSideRail component exists."
else
    echo -e "\e[33m[WARN]\e[0m Rail: RightSideRail component not yet injected. (Baseline state)"
fi

# 3. Audit the Substrate (Kernel/Backend)
# Check for binding in Rust server code
if grep -r "3000" bins/ide_backend/src/ > /dev/null; then
    echo -e "\e[32m[PASS]\e[0m Substrate: Kernel port 3000 binding detected."
else
    echo -e "\e[31m[FAIL]\e[0m Substrate: No binding to port 3000 found in src/."
fi

# 4. Audit DNA (Version/Build)
if [ -f "VERSION_CHECK.md" ]; then
    echo -e "\e[32m[PASS]\e[0m DNA: Version Check file present."
else
    echo -e "\e[31m[FAIL]\e[0m DNA: Missing version manifest."
fi

echo -e "\e[1;36m=== AUDIT COMPLETE ===\e[0m"

