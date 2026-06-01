#!/bin/bash

# Project Nexus Singularity - UI Upgrade Protocol
# Usage: ./upgrade_nexus_ui.sh

set -e

BASE_DIR="bins/ide_backend/ui"

echo "[*] Initializing Sovereign UI Upgrade..."

# 1. Cleanup old artifacts (Zero Fragmentation Protocol)
echo "[*] Flushing ARM64 binary fragments..."
rm -rf target/ *.so 2>/dev/null || true

# 2. Update Tailwind Palette
echo "[*] Updating Palette to 'Cool-Steel'..."
cat << 'EOF' > "$BASE_DIR/tailwind.config.js"
/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      colors: {
        'steel-dark': '#020617',
        'steel-accent': '#334155',
        'neural-cyan': '#22d3ee',
        'system-log': '#f8fafc',
      }
    }
  }
}
EOF

# 3. Inject PWA Manifest
echo "[*] Installing PWA Manifest..."
mkdir -p "$BASE_DIR/static"
cat << 'EOF' > "$BASE_DIR/static/manifest.json"
{
  "name": "AthenaFusionX",
  "short_name": "AthenaOS",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#020617",
  "theme_color": "#020617"
}
EOF

# 4. Global CSS Polish (Scrollbars)
echo "[*] Applying Global Scrollbar Override..."
cat << 'EOF' >> "$BASE_DIR/src/app.css"

/* AthenaOS System-Level Override */
* { scrollbar-width: thin; scrollbar-color: #334155 #020617; }
::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: #020617; }
::-webkit-scrollbar-thumb { background: #334155; border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: #22d3ee; }
EOF

# 5. Zero Fragmentation Sync Check
echo "[*] Validating Shared DNA..."
if command -v git &> /dev/null; then
    STATUS=$(git status --porcelain)
    if [ -n "$STATUS" ]; then
        echo "[!] WARNING: Uncommitted changes detected in UI layer."
    else
        echo "[+] GitHub sync status: CLEAN"
    fi
else
    echo "[!] Git not found. Skipping DNA sync check."
fi

echo "[+] Upgrade complete. Execute 'npm run build' to seat the new compositor."
