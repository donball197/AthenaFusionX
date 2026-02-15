#!/bin/bash
set -e  # Critical: Stop immediately if any command fails

# Cyberdeck Colors
CYAN='\033[1;36m'
GREEN='\033[1;32m'
RED='\033[1;31m'
NC='\033[0m' # No Color

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
# We suppress the noisy vite output unless there's an error
if npm run build > /dev/null; then
    echo -e "${GREEN}>> [VISOR] INTERFACE BUILT SUCCESSFULLY.${NC}"
else
    echo -e "${RED}>> [ERROR] VISOR BUILD FAILED.${NC}"
    exit 1
fi
cd ..

echo -e "${GREEN}>> [CORE]  IGNITING RUST ENGINE...${NC}"
# Low-memory mode for Tablet Hardware
export CARGO_BUILD_JOBS=1
cargo run --release
