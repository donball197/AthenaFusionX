#!/usr/bin/env bash

echo "AthenaFusionX Security Audit"

echo
echo "Checking unsafe Rust..."

grep -R "unsafe" crates \
    || echo "No unsafe blocks found"

echo
echo "Checking absolute path usage..."

grep -R 'Path::new("/")' crates \
    || echo "No direct root paths detected"

echo
echo "Checking command execution..."

grep -R "Command::new" crates \
    || echo "No direct command execution found"

