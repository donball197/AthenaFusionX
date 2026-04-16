#!/bin/bash
# Project Nexus Singularity - Sovereign Imprint v1.2 (Zig-Aware)
# Logic: Injects correct comment syntax based on file extension.

HEADER_C="/**
 * Copyright 2026 Donald Ball
 * Licensed under the Apache License, Version 2.0
 */"

HEADER_ZIG="// Copyright 2026 Donald Ball
// Licensed under the Apache License, Version 2.0"

FILES=$(find . -type f \( -name "*.rs" -o -name "*.zig" -o -name "*.ts" -o -name "*.tsx" -o -name "*.svelte" \) \
    -not -path "*/node_modules/*" \
    -not -path "*/target/*" \
    -not -path "*/.lazarus_vault/*" \
    -not -path "*/.git/*")

for f in $FILES; do
    if ! grep -q "Copyright 2026 Donald Ball" "$f"; then
        echo "Updating $f..."
        if [[ "$f" == *.zig ]]; then
            { echo "$HEADER_ZIG"; echo ""; cat "$f"; } > "$f.tmp" && mv "$f.tmp" "$f"
        else
            { echo "$HEADER_C"; echo ""; cat "$f"; } > "$f.tmp" && mv "$f.tmp" "$f"
        fi
    fi
done
echo "Injection complete."
