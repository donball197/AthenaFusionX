#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
echo ">>> [UPDATING CARGO MANIFEST]"
cargo add walkdir
cargo add futures-util
cargo add mime_guess
cargo add serde_json
cargo add tower-http --features "cors"
cargo add reqwest --features "json"
cargo add tokio --features "full"
echo ">>> [MANIFEST SYNCED]"
