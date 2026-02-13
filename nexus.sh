#!/bin/bash
# Project Nexus Singularity v0.2.8 - Pure Data CLI

ENDPOINT="http://localhost:3000/api"

case "$1" in
    wake|flush|vitals)
        >&2 echo "[NEXUS] Requesting $1..."
        # -w "\n" ensures the JSON is followed by a newline for the pipe
        curl -s -X POST "$ENDPOINT/$1" -w "\n"
        ;;
    audit)
        >&2 echo "[NEXUS] Initiating Audit..."
        curl -s -X POST -H "Content-Type: application/json" \
             -d '{"goal": "Deep Audit"}' "$ENDPOINT/mission" -w "\n"
        ;;
    *)
        >&2 echo "Usage: ./nexus.sh {wake|audit|flush|vitals}"
        exit 1
        ;;
esac
