#!/bin/bash
echo -e "\x1b[1;36m>> [ARCHITECT] Initiating Substrate Census...\x1b[0m"

curl -s -X POST http://localhost:3000/api/task \
     -H "Content-Type: application/json" \
     -d '{
       "directive": "Mission: Census. 1. Run a bash command to check the disk size of the file '\''nexus_memory.db'\'' and the folder '\''.lazarus_vault'\''. 2. Check current VmRSS using /proc/self/status. 3. Export a report titled '\''Substrate Census'\'' comparing your internal memory count to your physical RAM usage. Halt."
     }'

echo -e "\x1b[32m>> [SUCCESS] Census mission queued.\x1b[0m"
