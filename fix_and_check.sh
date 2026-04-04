#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
# 1. Inspect the generated index.html for pathing errors
echo ">>> [HTML LINK CHECK]"
grep -E "href|src" ui/dist/index.html | head -n 5

# 2. Check if assets are actually in the dist folder (not just the build cache)
echo ">>> [ASSET SEATING CHECK]"
ls -R ui/dist/_app/immutable/chunks | head -n 5

# 3. Check for Service Worker Ghosting
if [ ! -s ui/dist/service-worker.js ]; then
    echo "!!! WARNING: Service worker is empty. Forcing a dummy worker to prevent hydration lock."
    echo "self.addEventListener('install', () => self.skipWaiting());" > ui/dist/service-worker.js
fi

# 4. Clean up any stale Motorola artifacts that might have bled in
find ui/dist -name "*.gz" -delete
