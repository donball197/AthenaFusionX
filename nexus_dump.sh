#!/bin/bash

OUTDIR="nexus_dump"
CHUNK_SIZE=2000

mkdir -p "$OUTDIR"

echo "[+] Dumping files..."

dump_file () {
    FILE="$1"
    BASENAME=$(echo "$FILE" | sed 's/\//_/g')
    split -l $CHUNK_SIZE "$FILE" "$OUTDIR/${BASENAME}_part_"
}

export -f dump_file

find src ui/src -type f \( -name "*.rs" -o -name "*.svelte" -o -name "*.js" -o -name "*.ts" -o -name "*.zig" \) \
-exec bash -c 'dump_file "$0"' {} \;

echo "[✓] Done. Output in nexus_dump/"
