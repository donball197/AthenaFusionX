#!/bin/bash
echo "--- LATEST EVOLUTIONS ---"
git log -n 5 --pretty=format:"%h - %s (%cr)"
echo -e "\n\n--- CURRENT STATS ---"
cat .forge_stats.json | jq .
echo -e "\n--- PENDING CHANGES ---"
git diff HEAD~1
