#!/bin/bash
echo -e "\x1b[1;31m!! [WARGAME] Simulating a Substrate Fracture in alpha.txt...\x1b[0m"

# 1. Take a safe snapshot first
curl -s -X POST http://localhost:3000/api/task \
     -H "Content-Type: application/json" \
     -d '{ "directive": "Take a lazarus_snapshot of '\''alpha.txt'\'' tagged '\''pre_wargame'\'' and halt." }'

sleep 10

# 2. Corrupt the file manually
echo "SYSTEM_ERROR_CORRUPTED_DATA" > alpha.txt
echo "    -> alpha.txt has been corrupted."

# 3. Command the AI to find the hash and report the restore command
curl -s -X POST http://localhost:3000/api/task \
     -H "Content-Type: application/json" \
     -d '{ "directive": "Mission: Forensic Repair. 1. Detect corruption in alpha.txt. 2. Search the lazarus ledger for the '\''pre_wargame'\'' hash. 3. Use execute_command to restore the file from the backup path found in the ledger. 4. Verify alpha.txt is fixed and halt." }'

echo -e "\x1b[32m>> [SUCCESS] Healing sequence initiated.\x1b[0m"
