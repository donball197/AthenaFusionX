#!/bin/bash
# Re-compile Zig hooks
zig build-lib src/resonator.zig -lc -O ReleaseFast -femit-bin=target/release/libresonator.a
# Start Nexus
./target/release/nexus_singularity
