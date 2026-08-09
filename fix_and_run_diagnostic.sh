#!/bin/bash
set -e

echo ">> [NEXUS] Recompiling libnexus_loader.so with libc linkage (-lc)..."
zig build-lib lib/nexus_loader.zig -O ReleaseSmall -dynamic -fPIC -lc -femit-bin=lib/libnexus_loader.so

echo ">> [NEXUS] Executing BF16 Diagnostic Mapper..."
./inspect_bf16
