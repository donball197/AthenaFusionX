#!/bin/bash
echo -e "\x1b[1;33m[*] Testing Ghost Recall (VmoState Tracker)...\x1b[0m"

# Turn 1: Pulse and Write
curl -s -X POST http://localhost:3000/api/task \
     -H "Content-Type: application/json" \
     -d '{
       "directive": "Phase 1: Allocate a VMO pulse and write the string '\''GHOST_DNA_v2.1'\'' into it. Then halt immediately."
     }'

echo "    -> Waiting for Turn 1 to enshrine..."
sleep 15

# Turn 2: Recall and Read (The AI doesn't know the handle yet!)
curl -s -X POST http://localhost:3000/api/task \
     -H "Content-Type: application/json" \
     -d '{
       "directive": "Phase 2: Use '\''get_vmo_state'\'' to find the handle from the previous mission. Read that handle to verify it still contains '\''GHOST_DNA_v2.1'\''. Export a report titled '\''Recall Success'\''. Halt."
     }'

echo -e "\x1b[32m>> [SUCCESS] Multi-turn mission sequences initiated.\x1b[0m"
