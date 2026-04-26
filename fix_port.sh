#!/bin/bash
STALE_PID=$(lsof -t -i:3000)
if [ ! -z "$STALE_PID" ]; then
    echo ">> [SOVEREIGN] Severing stale connection on port 3000 (PID: $STALE_PID)"
    kill -9 $STALE_PID
fi
