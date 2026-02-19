#!/bin/bash
set -e 

# Cyberdeck Colors
CYAN='\033[1;36m'
GREEN='\033[1;32m'
RED='\033[1;31m'
NC='\033[0m'

clear
echo -e "${CYAN}"
cat << "BANNER"
   _  _  ____  _  _  _  _  ___ 
  ( \( )( ___)( \/ )/ )( \/ __)
   )  (  )__)  )  ( ) \/ (\__ \
  (_)\_)(____)(_/\_)\____/(___/
         SYSTEM ONLINE // v0.18.0
BANNER
echo -e "${NC}"

echo -e "${CYAN}>> [INIT]  CLEARING ORBITAL PATHS (PORT 3000)...${NC}"
fuser -k 3000/tcp 2>/dev/null || true

echo -e "${CYAN}>> [VISOR] COMPILING NEURAL INTERFACE...${NC}"
cd ui
if npm run build > /dev/null; then
    echo -e "${GREEN}>> [VISOR] INTERFACE BUILT SUCCESSFULLY.${NC}"
else
    echo -e "${RED}>> [ERROR] VISOR BUILD FAILED.${NC}"
    exit 1
fi
cd ..

echo -e "${CYAN}>> [OPTIC] PRIMING ZIG SIMD CO-PROCESSOR...${NC}"
# This ensures the "Muscle" is always ready for the "Nerve"
zig build-exe src/vision_proc.zig -O ReleaseSmall -target aarch64-linux --name vision_proc

echo -e "${GREEN}>> [CORE]  IGNITING RUST ENGINE...${NC}"
export CARGO_BUILD_JOBS=1
cargo run --release
