#!/bin/bash
echo ">> [SUTURE] Removing syntax fractures from source..."

# Fix src/server.rs
sed -i 's/use ;//g' src/server.rs

# Fix src/self_healing_engine.rs
sed -i 's/use ;//g' src/self_healing_engine.rs

echo ">> [SUCCESS] Fractures cleared."
