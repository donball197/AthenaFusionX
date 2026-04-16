# Project Nexus Singularity - Ziggy-LLM Muscle Manifest
## Substrate: ARM64 (Chromebook/Motorola)
## Version: v1.9 (Reflex Layer)

### Core Components
* **gguf.zig**: Native parser for Gemma 4 GGUF headers.
* **moon_quant_runtime.zig**: High-speed JIT for qint4/qint8 quantized tensors.
* **llama_cpu.zig**: ARM64 NEON optimized compute kernels.
* **sampling.zig**: Greedy and Top-P sampling for the reflex loop.

### Knowledge Paths
* **Model Seat**: `./models/google_gemma-4-E2B-it-Q4_K_M.gguf`
* **Inference Entry**: `src/main.zig`
* **Shared DNA**: Synced via GitHub to `nexus_singularity-dev` root.

### Operational Intent
The Ziggy-LLM layer provides "Pure Muscle" execution. It is synthesized by the Strategic Brain to perform low-latency tasks on the Motorola metal.
