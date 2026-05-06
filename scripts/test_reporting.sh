#!/bin/bash
echo -e "\x1b[1;33m[*] Testing Executor Reporting capability...\x1b[0m"

curl -s -X POST http://localhost:3000/api/task \
     -H "Content-Type: application/json" \
     -d '{
       "directive": "Write a report titled '\''Connection Test'\'' containing the text '\''SUBSTRATE_LINK_OK'\'' and then halt."
     }'

echo -e "\n[*] Waiting 10 seconds..."
sleep 10

if [ -f "exports/AUDIT_Connection_Test.md" ]; then
    echo -e "\x1b[32m>> [SUCCESS] Executor is physically writing to disk.\x1b[0m"
    cat exports/AUDIT_Connection_Test.md
else
    echo -e "\x1b[31m!! [FAILURE] Executor failed to write report. Check nexus-dev terminal for errors.\x1b[0m"
fi
