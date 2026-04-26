<script lang="ts">
    // TrinityMatrix.svelte — v1.1
    // FIX: No longer assumes compute_thermal/compute_hash exist.
    //      Discovers exported functions at load time and uses the first
    //      two available. Shows a clear warning if exports don't match.
    import { onMount, onDestroy } from "svelte";
    import { systemVitals, terminalState } from "$lib/core/store.svelte";
    import { TaskService } from '$lib/services/TaskService';

    let wasmEngine   = $state<Record<string, Function> | null>(null);
    let exportNames  = $state<string[]>([]);
    let warnMsg      = $state('');
    let hexGrid      = $state<string[]>(Array(64).fill("00"));

    let cpu     = $derived(systemVitals.cpu_load   || 0);
    let temp    = $derived(systemVitals.temperature || 30);
    let lastLog = $derived(terminalState.logs.at(-1)?.message || "");

    // Derived outputs — only compute when engine is loaded
    let thermalVal = $derived.by(() => {
        if (!wasmEngine || exportNames.length < 1) return 0;
        try { return (wasmEngine[exportNames[0]] as Function)(cpu, temp) as number; }
        catch { return 0; }
    });
    let hashVal = $derived.by(() => {
        if (!wasmEngine || exportNames.length < 2) return 0;
        try { return (wasmEngine[exportNames[1]] as Function)(cpu, temp) as number; }
        catch { return 0; }
    });

    async function handleGenesis(event: Event) {
        const b64 = (event as CustomEvent<string>).detail;
        try {
            TaskService.log(">> [TRINITY] Intercepting WASM logic core...", "text-purple-400");
            const binaryString = window.atob(b64);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) bytes[i] = binaryString.charCodeAt(i);

            const { instance } = await WebAssembly.instantiate(bytes.buffer);
            const exports = instance.exports as Record<string, unknown>;

            // Discover function exports
            const fns = Object.entries(exports)
                .filter(([, v]) => typeof v === 'function')
                // skip internal WASM bookkeeping exports
                .filter(([k]) => !k.startsWith('__') && !k.startsWith('_'))
                .map(([k]) => k);

            if (fns.length === 0) {
                warnMsg = 'No exported functions found in this WASM cell.';
                TaskService.log("!! [TRINITY] No usable exports in cell.", "text-rose-400");
                return;
            }

            wasmEngine  = exports as Record<string, Function>;
            exportNames = fns;
            warnMsg     = fns.length < 2 ? `Only 1 export found: ${fns[0]}` : '';
            TaskService.log(
                `>> [TRINITY] Bound ${fns.length} export(s): ${fns.slice(0,3).join(', ')}`,
                "text-emerald-400"
            );
        } catch (e: any) {
            warnMsg = `V8 Engine Fault: ${e.message}`;
            TaskService.log(`!! [TRINITY] ${e.message}`, "text-rose-400");
        }
    }

    $effect(() => {
        if (!wasmEngine || !lastLog || exportNames.length < 2) return;
        const charCode = lastLog.charCodeAt(0) || 1;
        try {
            const entropy = (wasmEngine[exportNames[1]] as Function)(hashVal, charCode) as number;
            hexGrid = hexGrid.map((_, i) => {
                const shift = (entropy * (i + 1) * 100) % 255;
                return Math.floor(Math.abs(shift)).toString(16).padStart(2,'0').toUpperCase();
            });
        } catch { /* non-matching function signature — ignore */ }
    });

    onMount(()  => window.addEventListener("nexus-genesis", handleGenesis));
    onDestroy(() => window.removeEventListener("nexus-genesis", handleGenesis));
</script>

<div class="trinity-root">
    <div class="header">◈ TRINITY_MATRIX // NATIVE_WASM_ACCELERATED</div>

    {#if warnMsg}
        <div class="warn-banner">⚠ {warnMsg}</div>
    {/if}

    {#if wasmEngine && exportNames.length >= 1}
        <div class="metrics">
            <div class="metric-box" style="border-color:{thermalVal > 0.8 ? '#f87171' : '#34d399'}">
                <span class="lbl">{exportNames[0]?.toUpperCase() ?? 'FN_0'}</span>
                <span class="val" style="color:{thermalVal > 0.8 ? '#f87171' : '#34d399'}">
                    {(thermalVal * 100).toFixed(1)}%
                </span>
            </div>
            {#if exportNames.length >= 2}
                <div class="metric-box border-purple-500/50">
                    <span class="lbl">{exportNames[1]?.toUpperCase() ?? 'FN_1'}</span>
                    <span class="val text-purple-400">{hashVal.toFixed(4)}</span>
                </div>
            {/if}
        </div>
        <div class="hex-grid">
            {#each hexGrid as hex}
                <div class="hex-cell" style="opacity:{parseInt(hex, 16) / 255}">{hex}</div>
            {/each}
        </div>
    {:else}
        <div class="standby">
            {#if warnMsg}
                CELL LOADED — INCOMPATIBLE EXPORTS
            {:else}
                AWAITING KERNEL_GENESIS PAYLOAD...
            {/if}
        </div>
    {/if}
</div>

<style>
    .trinity-root { padding:20px; background:#020208; font-family:'JetBrains Mono',monospace; height:100%; display:flex; flex-direction:column; gap:20px; }
    .header { color:#a78bfa; font-size:14px; font-weight:900; letter-spacing:.2em; border-bottom:1px solid rgba(167,139,250,.2); padding-bottom:10px; }
    .warn-banner { background:rgba(251,191,36,.05); border:1px solid rgba(251,191,36,.3); color:#fbbf24; font-size:9px; padding:6px 10px; }
    .metrics { display:flex; gap:20px; }
    .metric-box { flex:1; padding:15px; border:1px solid; background:rgba(0,0,0,.5); display:flex; flex-direction:column; gap:5px; }
    .lbl { font-size:10px; color:#64748b; font-weight:bold; }
    .val { font-size:24px; font-weight:900; }
    .hex-grid { display:grid; grid-template-columns:repeat(16,1fr); gap:2px; flex:1; }
    .hex-cell { background:#22d3ee; color:#000; display:flex; align-items:center; justify-content:center; font-size:10px; font-weight:bold; transition:opacity .1s; }
    .standby { flex:1; display:flex; align-items:center; justify-content:center; color:#64748b; font-size:12px; letter-spacing:.1em; animation:pulse 2s infinite; }
    @keyframes pulse { 50%{opacity:.5;} }
</style>
