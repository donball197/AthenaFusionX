#!/bin/bash
# Project Nexus Singularity - Sovereign Imprint v1.1
# Logic: Automatically injects Apache 2.0 headers into anonymous strands.

HEADER_TEXT="/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the \"License\");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an \"AS IS\" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */"

# Target active logic strands, EXCLUDING snapshots and build artifacts
FILES=$(find . -type f \( -name "*.rs" -o -name "*.zig" -o -name "*.ts" -o -name "*.tsx" -o -name "*.svelte" \) \
    -not -path "*/node_modules/*" \
    -not -path "*/target/*" \
    -not -path "*/.lazarus_vault/*" \
    -not -path "*/.git/*")

echo -e "\x1b[36m>> [IMPRINT] Commencing DNA injection... \x1b[0m"

for f in $FILES
do
    if ! grep -q "Copyright 2026 Donald Ball" "$f"; then
        echo -e "\x1b[32m[IMPRINTING]\x1b[0m $f"
        # Create temp file with header + original content
        echo "$HEADER_TEXT" > "$f.tmp"
        echo -e "\n" >> "$f.tmp"
        cat "$f" >> "$f.tmp"
        mv "$f.tmp" "$f"
    fi
done

echo -e "\x1b[35m>> [COMPLETE] Active strands are now legally bound to Nexus Singularity.\x1b[0m"
