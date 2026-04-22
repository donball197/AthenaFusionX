<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { systemVitals, terminalState } from "$lib/core/store.svelte.js";
    import { pushTermLine } from "$lib/core/NexusWS.js";

    let wasmEngine = $state<any>(null);
    let hexGrid = $state<string[]>(Array(64).fill("00"));
    
    // Live reactive inputs
    let cpu = $derived(systemVitals.cpu_load || 0);
    let temp = $derived(systemVitals.temperature || 30);
    let latestLog = $derived(terminalState.logs.at(-1)?.message || "");

    // WASM computed outputs
    let thermalThreshold = $derived(wasmEngine ? wasmEngine.compute_thermal(cpu, temp) : 0);
    let hashSeed = $derived(wasmEngine ? wasmEngine.compute_hash(cpu, temp) : 0);

    async function handleGenesis(event: CustomEvent<string>) {
        try {
            pushTermLine(">> [TRINITY] Intercepting WASM logic core...", "text-purple-400");
            const binaryString = window.atob(event.detail);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) bytes[i] = binaryString.charCodeAt(i);
            
            const { instance } = await WebAssembly.instantiate(bytes.buffer);
            wasmEngine = instance.exports;
            pushTermLine(">> [TRINITY] C-ABI bindings secured. Native logic online.", "text-emerald-400");
        } catch (e: any) {
            pushTermLine(`!! [TRINITY] V8 Engine Fault: ${e.message}`, "text-rose-400");
        }
    }

    // Mathematical visualizer loop
    $effect(() => {
        if (!wasmEngine || !latestLog) return;
        const charCode = latestLog.charCodeAt(0) || 1;
        const entropy = wasmEngine.compute_hash(hashSeed, charCode);
        
        hexGrid = hexGrid.map((val, i) => {
            const shift = (entropy * (i + 1) * 100) % 255;
            return Math.floor(shift).toString(16).padStart(2, '0').toUpperCase();
        });
    });

    onMount(() => window.addEventListener("nexus-genesis", handleGenesis as EventListener));
    onDestroy(() => {
        if (typeof window !== 'undefined') window.removeEventListener("nexus-genesis", handleGenesis as EventListener);
    });
</script>

<div class="trinity-root">
    <div class="header">◈ TRINITY_MATRIX // NATIVE_WASM_ACCELERATED</div>
    
    {#if wasmEngine}
        <div class="metrics">
            <div class="metric-box" style="border-color: {thermalThreshold > 0.8 ? '#f87171' : '#34d399'}">
                <span class="lbl">THERMAL_GOVERNOR</span>
                <span class="val" style="color: {thermalThreshold > 0.8 ? '#f87171' : '#34d399'}">
                    {(thermalThreshold * 100).toFixed(1)}%
                </span>
            </div>
            <div class="metric-box border-purple-500/50">
                <span class="lbl">HASH_ENTROPY</span>
                <span class="val text-purple-400">{hashSeed.toFixed(4)}</span>
            </div>
        </div>

        <div class="hex-grid">
            {#each hexGrid as hex}
                <div class="hex-cell" style="opacity: {parseInt(hex, 16) / 255}">{hex}</div>
            {/each}
        </div>
    {:else}
        <div class="standby">AWAITING KERNEL_GENESIS PAYLOAD...</div>
    {/if}
</div>

<style>
    .trinity-root { padding: 20px; background: #020208; font-family: 'JetBrains Mono', monospace; height: 100%; display: flex; flex-direction: column; gap: 20px; }
    .header { color: #a78bfa; font-size: 14px; font-weight: 900; letter-spacing: 0.2em; border-bottom: 1px solid rgba(167, 139, 250, 0.2); padding-bottom: 10px; }
    .metrics { display: flex; gap: 20px; }
    .metric-box { flex: 1; padding: 15px; border: 1px solid; background: rgba(0,0,0,0.5); display: flex; flex-direction: column; gap: 5px; }
    .lbl { font-size: 10px; color: #64748b; font-weight: bold; }
    .val { font-size: 24px; font-weight: 900; }
    .hex-grid { display: grid; grid-template-columns: repeat(16, 1fr); gap: 2px; flex: 1; }
    .hex-cell { background: #22d3ee; color: #000; display: flex; align-items: center; justify-content: center; font-size: 10px; font-weight: bold; transition: opacity 0.1s; }
    .standby { flex: 1; display: flex; align-items: center; justify-content: center; color: #64748b; font-size: 12px; letter-spacing: 0.1em; animation: pulse 2s infinite; }
    @keyframes pulse { 50% { opacity: 0.5; } }
</style>
