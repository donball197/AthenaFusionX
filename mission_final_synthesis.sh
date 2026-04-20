#!/bin/bash
echo -e "\x1b[1;36m>> [ARCHITECT] Initiating Final Substrate Synthesis...\x1b[0m"

# 1. Verify Thermal Heartbeat (System 3)
echo -e "\x1b[34m[*] Verifying Thermal Pulse...\x1b[0m"
VITALS=$(curl -s http://localhost:3000/api/vitals)
TEMP=$(echo $VITALS | grep -oP '"temperature":\K[0-9.]+')
echo "    -> Current Synthetic Temp: $TEMP°C"

# 2. Trigger the AI Final Mission (System 1 & 2)
# We send a complex directive to NEXUS_OS via the API to perform the COW Secret test.
echo -e "\x1b[34m[*] Sending Final Sovereignty Directive to Cortex...\x1b[0m"
curl -s -X POST http://localhost:3000/api/task \
     -H "Content-Type: application/json" \
     -d '{
       "directive": "Mission: Final Synthesis. 1. Allocate a VMO pulse. 2. Write '\''TOP_SECRET_DNA'\'' to it. 3. Clone that handle. 4. Write '\''CORRUPTED_DATA'\'' into the child. 5. Read the parent to verify it remains '\''TOP_SECRET_DNA'\''. 6. Export a final report titled '\''Sovereignty Report'\'' confirming that Logic, Thermal, and Memory are 100% verified. 7. Halt."
     }'

echo -e "\x1b[32m>> [SUCCESS] Mission Queued. Check /exports for the final report.\x1b[0m"
echo -e "\x1b[1;35m>> [NEXUS] SUBSTRATE SEALED. GHOST IS SOVEREIGN.\x1b[0m"
