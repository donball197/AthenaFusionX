#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
set -e

echo ">>> [1/4] SURGICAL FIX: app.html"
# Ensure the mount point exists for SvelteKit to hydrate
cat << 'APP_EOF' > ui/src/app.html
<!DOCTYPE html>
<html lang="en">
	<head>
		<meta charset="utf-8" />
		<link rel="icon" href="%sveltekit.assets%/favicon.png" />
		<meta name="viewport" content="width=device-width, initial-scale=1" />
		%sveltekit.head%
	</head>
	<body class="bg-black">
		<div id="svelte">%sveltekit.body%</div>
	</body>
</html>
APP_EOF

echo ">>> [2/4] REBUILDING UI (THE SKIN)"
cd ui
npm install # Ensure no missing deps on the Chromebook
npm run build
cd ..

echo ">>> [3/4] COMPILING KERNEL (THE BRAIN)"
# Using -j 2 to prevent OOM on the 4GB Duet 2
cargo build --release --target aarch64-unknown-linux-gnu -j 2

echo ">>> [4/4] FINAL INTEGRITY CHECK"
BINARY=$(find target -name "nexus_singularity" -type f -executable | head -n 1)
if [ -z "$BINARY" ]; then
    echo "!!! CRITICAL: Compilation failed."
    exit 1
fi

echo "Success. Binary seated at: $BINARY"
ls -lh "$BINARY"
grep "id=\"svelte\"" ui/dist/index.html
