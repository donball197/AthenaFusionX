#!/bin/bash

find . -type f \( -name "*.rs" -o -name "*.toml" -o -name "*.zig" \) \
  -not -path "*/.*" \
  -not -path "*/target/*" \
  -not -path "*/node_modules/*" \
  | sort | while read -r file; do
    echo "========================================"
    echo "[FILE: $file] ($(wc -l < "$file") lines)"
    echo "========================================"
    cat "$file"
    echo ""
  done > custom_codebase_audit.txt

echo "Done! Output written to custom_codebase_audit.txt"
