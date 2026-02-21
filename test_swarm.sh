#!/bin/bash
echo ">> [HIVE TEST] Verifying Forensics Bridge..."

# 1. Check if the server is up
curl -s http://localhost:3000/api/vitals > /dev/null
if [ $? -eq 0 ]; then
    echo "[OK] Hive Queen is Online."
else
    echo "[FAIL] Hive Queen is Offline. Launch ./target/release/nexus_singularity first."
    exit 1
fi

# 2. Check the SecOps Drone
echo ">> [HIVE TEST] Attempting to Spawn SecOps Specialist..."
curl -X POST -H "Content-Type: application/json" \
     -d '{"drone": "secops", "action": "spawn"}' \
     http://localhost:3000/api/hive/spawn

# 3. Test the Dissector
echo -e "\n>> [HIVE TEST] Running Forensic Autopsy on Server Binary..."
curl -X POST -H "Content-Type: application/json" \
     -d '{"prompt": "dissect target/release/nexus_singularity"}' \
     http://localhost:3000/api/prompt
