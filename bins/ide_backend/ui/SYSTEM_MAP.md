=========================================================
>> PROJECT NEXUS SINGULARITY v0.2.2 - SYSTEM ARCHITECTURE
=========================================================
ARCHITECT: Donald Ball
TARGET HARDWARE: 3nm ARM64 (Motorola Laguna G5 / Crostini)
EFFICIENCY: 50% Electrical OpEx Reduction (Zero Local Sprawl)

[1] THE EDGE FRONTEND (SvelteKit / CodeMirror 6)
---------------------------------------------------------
ui/src/routes/+page.svelte        # The Master UI Layout / State Manager
ui/src/lib/components/
 ├── Editor.svelte                # CodeMirror 6 Core w/ GhostCortex
 ├── TacticalBlade.svelte         # Ctrl+K Omnibox & File Forge
 ├── ResponseModule.svelte        # Actionable Merge Gate (Auto-Patching)
 ├── Terminal.svelte              # xterm.js WebSocket PTY
 ├── Vitals.svelte                # Real-time Hardware Telemetry
 └── SwarmMap.svelte              # Podman Hive Visualizer

[2] THE HYBRID BACKEND (Memory-Safe Rust / Axum)
---------------------------------------------------------
src/
 ├── main.rs                      # Ignition, Watchdogs, & Shared DNA Validation
 ├── server.rs                    # Axum REST/WebSocket Routes & FS Control
 ├── neural_agent.rs              # STRICT API ROUTING (Gemma 3 27B-it)
 ├── podman.rs                    # Local Container Orchestration (SecOps/DevOps)
 ├── system_monitor.rs            # Hardware Vitals Polling
 ├── industrial.rs                # Auto-Archiving & Throttle Logic
 └── hiram_core.rs                # 13th Layer Auto-Healer

[3] THE INFRASTRUCTURE ("Minus Kali" Baseline)
---------------------------------------------------------
Dockerfile                        # Alpine / aarch64-unknown-linux-musl
universal_ignite.sh               # Environment Drift Prevention
nexus.state                       # Persistent System Memory

=========================================================
STATUS: 100% DETERMINISTIC. ZERO FRAGMENTATION. 
API ROUTING: STRICT. 
=========================================================
