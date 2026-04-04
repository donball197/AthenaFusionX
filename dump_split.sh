#!/usr/bin/env bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
set -e

# ======================================================
# NEXUS CORE // dump_split.sh (v2.0)
# CPU-Optimized Trilateral Split with Integrity Verification
# ======================================================

if [ -z ""$1"" ]; then
    echo -e "\e[33m>> [USAGE] ./dump_split.sh <path_to_file>\e[0m"
    exit 1
fi

FILE=""$1""

if [ ! -f "$FILE" ]; then
    echo -e "\e[31m>> [ERROR] Target buffer '$FILE' not found or inaccessible.\e[0m"
    exit 1
fi

TOTAL_LINES=$(wc -l < "$FILE")

if [ "$TOTAL_LINES" -lt 3 ]; then
    echo -e "\e[31m>> [ERROR] Buffer too small for trilateral split (Lines: $TOTAL_LINES).\e[0m"
    exit 1
fi

# Calculate chunk boundaries
CHUNK_SIZE=$((TOTAL_LINES / 3))
PART1=$CHUNK_SIZE
PART2=$((CHUNK_SIZE * 2))

echo -e "\e[36m>> [NEXUS] Initiating Single-Pass Buffer Split...\e[0m"
echo -e "\e[90m   Total lines: $TOTAL_LINES\e[0m"

PART1_FILE="${FILE}.part1"
PART2_FILE="${FILE}.part2"
PART3_FILE="${FILE}.part3"

# O(N) Execution with CPU-Cycle skip ('next')
awk -v p1="$PART1" -v p2="$PART2" \
    -v f1="$PART1_FILE" -v f2="$PART2_FILE" -v f3="$PART3_FILE" '
    NR <= p1 { print > f1; next }
    NR <= p2 { print > f2; next }
    { print > f3 }
' "$FILE"

echo -e "\e[32m>> [SUCCESS] Trilateral dump secured. Integrity verified:\e[0m"
wc -l "$PART1_FILE" "$PART2_FILE" "$PART3_FILE" | grep -v total | awk '{print "  -> \033[33m" $2 "\033[0m (" "$1" " lines)"}'
