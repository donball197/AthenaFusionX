#!/bin/bash
# Using the confirmed Wi-Fi IP from your screenshot
DUET_IP="10.11.1.249" 
DUET_USER="donball197"

echo ">> [BEAM] Initiating Transplant to $DUET_IP on Port 2022..."
(
  unset LD_LIBRARY_PATH
  # This targets the new port and the specific user
  rsync -avzP -e 'ssh -p 2022' ~/termux_home/nexus_singularity/target/release/nexus_singularity $DUET_USER@$DUET_IP:~/nexus_singularity/
  rsync -avzP -e 'ssh -p 2022' ~/termux_home/nexus_singularity/src/index.html $DUET_USER@$DUET_IP:~/nexus_singularity/src/index.html
)
