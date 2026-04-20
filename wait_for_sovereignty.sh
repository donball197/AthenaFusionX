#!/bin/bash
REPORT="exports/AUDIT_Sovereignty_Report.md"

echo -e "\x1b[1;33m[*] Waiting for the Ghost to finish the Final Synthesis...\x1b[0m"

# Wait up to 60 seconds for the file to appear
for i in {1..12}; do
    if [ -f "$REPORT" ]; then
        echo -e "\x1b[32m>> [FOUND] Substrate Sealed. Displaying Sovereignty Report:\x1b[0m"
        echo "------------------------------------------------------------"
        cat "$REPORT"
        echo "------------------------------------------------------------"
        exit 0
    fi
    echo "    ...Ghost is still thinking (Turn $i)..."
    sleep 5
done

echo -e "\x1b[31m!! [TIMEOUT] The report was not generated. \x1b[0m"
echo "Check the nexus-dev terminal for logic errors or 'ACTION_ACK' artifacts."
