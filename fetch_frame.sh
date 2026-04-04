#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
# 1. Clear the specific path to prevent append/corruption errors
rm -f /sdcard/vision_input.jpg

# 2. Capture the new frame
termux-camera-photo -c 0 /sdcard/vision_input.jpg

# 3. Clean up the Termux temporary folder just in case
rm -rf $TMPDIR/*

echo "[$(date +%T)] Vision frame refreshed (Old data purged)."
