#!/usr/bin/env bash
# NEXUS SINGULARITY - ARCHITECTURE DASHBOARD
set -e

echo -e "\n\e[1;36m============================================================\e[0m"
echo -e "\e[1;36m      NEXUS SINGULARITY v1.9 — SOVEREIGN ARCHITECTURE       \e[0m"
echo -e "\e[1;36m============================================================\e[0m"

# 1. THE SUBSTRATE (Rust Core Modules)
echo -e "\n\e[35m>> [THE RUST SUBSTRATE] (Top 10 Heaviest Modules)\e[0m"
if ls src/*.rs 1> /dev/null 2>&1; then
    wc -l src/*.rs | sort -nr | head -n 11 | grep -v "total" | awk '{printf "   \033[33m%4s\033[0m lines | \033[37m%s\033[0m\n", $1, $2}'
else
    echo "   No source files detected in src/"
fi

# 2. THE PHYSICAL MARROW (Local ARM64 Libraries)
echo -e "\n\e[32m>> [THE PHYSICAL MARROW] (ARM64 DNA & Shared Libraries)\e[0m"
if [ -d "lib" ] && [ "$(ls -A lib)" ]; then
    ls -lh lib/ | tail -n +2 | awk '{printf "   \033[36m%-8s\033[0m | \033[37m%s\033[0m\n", $5, $9}'
else
    echo -e "   \e[90mNo physical .so libraries seated in lib/\e[0m"
fi

# 3. THE LAZARUS VAULT (SQLite Memory Palace)
echo -e "\n\e[38;5;208m>> [THE LAZARUS VAULT] (Memory & Neural Context)\e[0m"
if ls *.db 1> /dev/null 2>&1; then
    ls -lh *.db | awk '{printf "   \033[33m%-8s\033[0m | \033[37m%s\033[0m\n", $5, $9}'
else
    echo -e "   \e[90mVault not initialized yet.\e[0m"
fi

# 4. THE TRI-CORE ROUTER (Active AI Endpoints)
echo -e "\n\e[38;5;51m>> [THE TRI-CORE ROUTER] (Cognitive Network Status)\e[0m"
if [ -f "src/agent_protocol.rs" ]; then
    grep -E "gemini-2.5-flash|gemma-4-31b-it|local_reflex" src/agent_protocol.rs | sed 's/^[ \t]*//' | awk '{printf "   \033[32mACTIVE:\033[0m \033[37m%s\033[0m\n", $0}' | head -n 3
else
    echo -e "   \e[90mRouter offline or protocol missing.\e[0m"
fi

# 5. SHARED DNA STATUS (Git Synchronization)
echo -e "\n\e[31;1m>> [SHARED DNA STATUS] (Uncommitted Forge Operations)\e[0m"
if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    git status -s | awk '{printf "   \033[31m%s\033[0m \033[37m%s\033[0m\n", $1, $2}'
    if [ -z "$(git status -s)" ]; then
        echo -e "   \e[32m[SYNC_NOMINAL] All DNA synchronized to GitHub.\e[0m"
    fi
else
    echo -e "   \e[90mGit repository not initialized.\e[0m"
fi

echo -e "\e[1;36m============================================================\e[0m\n"
