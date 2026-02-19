# NEXUS SINGULARITY BUILD PROTOCOL

.PHONY: all ui server clean

all: ui server

ui:
	@echo ">> [NEXUS] Compiling Visual Cortex (Svelte)..."
	cd ui && npm install && npm run build

server:
	@echo ">> [NEXUS] Forging Kernel (Rust Monolith)..."
	cargo build --release

run:
	@echo ">> [NEXUS] IGNITION..."
	./target/release/nexus_singularity

clean:
	cargo clean
	rm -rf ui/dist
