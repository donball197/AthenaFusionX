#!/usr/bin/env bash
# NEXUS SINGULARITY - ARCHITECTURE EXPORT PROTOCOL
set -e

BACKEND_OUT="nexus_full_backend_review.txt"
FRONTEND_OUT="nexus_full_frontend_review.txt"

echo -e "\x1b[1;36m>> [NEXUS] Initiating full architecture telemetry sweep...\x1b[0m"

append_file() {
    local file="$1"
    local out="$2"
    if [ -f "$file" ]; then
        echo -e "\n\n/// --- FILE: $file --- ///" >> "$out"
        cat "$file" >> "$out"
    else
        echo -e "\n\n/// --- FILE: $file (NOT FOUND) --- ///" >> "$out"
    fi
}

echo "=== NEXUS SINGULARITY - FULL BACKEND REVIEW ===" > "$BACKEND_OUT"
echo "Generated: $(date)" >> "$BACKEND_OUT"
echo "Version Baseline: v1.9" >> "$BACKEND_OUT"

append_file "Cargo.toml" "$BACKEND_OUT"
append_file "build.rs" "$BACKEND_OUT"

for f in src/*.rs; do
    [ -f "$f" ] && append_file "$f" "$BACKEND_OUT"
done

for f in src/kernel/*.rs; do
    [ -f "$f" ] && append_file "$f" "$BACKEND_OUT"
done

for f in lib/*; do
    [ -f "$f" ] && append_file "$f" "$BACKEND_OUT"
done

echo "=== NEXUS SINGULARITY - FULL FRONTEND REVIEW ===" > "$FRONTEND_OUT"
echo "Generated: $(date)" >> "$FRONTEND_OUT"
echo "Framework: Svelte 5 / Vite" >> "$FRONTEND_OUT"

append_file "ui/package.json" "$FRONTEND_OUT"
append_file "ui/vite.config.js" "$FRONTEND_OUT"
append_file "ui/svelte.config.js" "$FRONTEND_OUT"
append_file "ui/src/routes/+layout.svelte" "$FRONTEND_OUT"
append_file "ui/src/routes/+page.svelte" "$FRONTEND_OUT"

if ls ui/src/lib/components/*.svelte 1> /dev/null 2>&1; then
    for f in ui/src/lib/components/*.svelte; do
        [ -f "$f" ] && append_file "$f" "$FRONTEND_OUT"
    done
fi

echo -e "\x1b[32m>> [SUCCESS] Telemetry generated: $BACKEND_OUT and $FRONTEND_OUT\x1b[0m"
