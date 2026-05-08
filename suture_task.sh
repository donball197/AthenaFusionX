#!/bin/bash
# ⚡ NEXUS AUTONOMOUS SUTURE LOOP

LOG_FILE="build_failure.log"

echo ">> Initiating Integrity Check..."
cargo check 2> $LOG_FILE

if [ $? -eq 0 ]; then
    echo -e "\033[32m>> Integrity Verified. No suture required.\033[0m"
    rm $LOG_FILE
else
    echo -e "\033[31m>> FRACTURE DETECTED. Engaging API RepoDoctor...\033[0m"
    
    # 1. Harvest latest DNA
    ./harvest_dna.sh
    
    # 2. Construct the Repair Directive
    ERROR_DATA=$(cat $LOG_FILE)
    DNA_DATA=$(cat workspace_dna.txt)
    
    cat <<INNER_EOF > repair_directive.json
{
  "directive": "The project failed to compile with the following error: $ERROR_DATA. Using the provided Workspace DNA, identify the file and apply a diff_patch to fix the error. Ensure you follow the Execution Law.",
  "context_dna": "$DNA_DATA"
}
INNER_EOF

    echo ">> Repair Directive generated. Submitting to NEXUS IDE Backend..."
    curl -X POST -H "Content-Type: application/json" \
         -d @repair_directive.json \
         http://localhost:3000/api/task
fi
