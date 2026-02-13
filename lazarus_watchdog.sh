#!/bin/bash
while true; do
  # Get available memory in MB
  AVAILABLE=$(free -m | awk '/^Mem:/{print $7}')
  
  if [ "$AVAILABLE" -lt 500 ]; then
    echo ">> [LAZARUS] CRITICAL RAM: ${AVAILABLE}MB. TRIGGERING AUTO-FLUSH..."
    curl -s -X POST http://127.0.0.1:3000/api/flush > /dev/null
  fi
  
  sleep 10
done
