<script>
    /**
     * AgentLab.svelte — v1.3 [UPLINK_SUTURE]
     * FIX: Now uses the centralized uplink service for authenticated requests.
     */
    import { onMount, onDestroy } from 'svelte';
    import { fly } from 'svelte/transition';
    import { TaskService } from '$lib/services/TaskService';
    import { uplink } from '$lib/services/uplink.js';

    let cluster   = $state({ best_node: null, has_capacity: false });
    let wasmCells = $state([]);
    let loading   = $state(true);
    let pollTimer;

    async function fetchMeshState() {
        if (document.visibilityState === 'hidden') return;
        try {
            const [status, cells] = await Promise.all([
                uplink.json('/api/cluster/status'),
                uplink.json('/api/cluster/cells')
            ]);
            cluster = status;
            wasmCells = cells.cells || [];
        } catch (e) {
            console.warn('[MESH] Telemetry error:', e.message);
        } finally {
            loading = false;
        }
    }

    onMount(() => {
        fetchMeshState();
        pollTimer = setInterval(fetchMeshState, 3000);
    });

    onDestroy(() => clearInterval(pollTimer));

    async function executeCell(cellName) {
        TaskService.log(`>> [MESH] Executing cell: ${cellName}`, 'text-purple-400');
        await TaskService.runDirective(
            `Load the WASM cell named "${cellName}" from the registry using wasm_load. Halt when finished.`,
            'AGENT_LAB'
        );
    }
</script>

<div class="h-full bg-black font-mono flex flex-col p-6 gap-6 overflow-hidden">
    <header class="flex justify-between items-end border-b border-cyan-500/20 pb-4">
        <div>
            <h1 class="text-cyan-400 text-xl font-black tracking-widest italic">DISTRIBUTED_MESH</h1>
            <p class="text-cyan-900 text-[9px]">P2P ARM64 WORKLOAD ROUTING</p>
        </div>
        <div class="text-[10px]">
            {#if cluster.has_capacity}
                <span class="text-emerald-400">● MESH_ACTIVE</span>
            {:else}
                <span class="text-rose-500">○ STANDALONE_MODE</span>
            {/if}
        </div>
    </header>

    <div class="flex-1 flex gap-6 min-h-0">
        <aside class="w-1/3 space-y-4">
            <h2 class="text-cyan-700 text-[10px] font-bold tracking-widest border-b border-cyan-900/30 pb-1">TARGET_NODE</h2>
            {#if cluster.best_node}
                <div class="bg-cyan-950/10 border border-cyan-500/30 p-4">
                    <div class="text-cyan-400 font-bold mb-2">{cluster.best_node.id}</div>
                    <div class="text-[9px] space-y-1 text-gray-500">
                        <div class="flex justify-between"><span>CPU</span><span>{cluster.best_node.cpu}%</span></div>
                        <div class="flex justify-between"><span>TEMP</span><span>{cluster.best_node.temp}°C</span></div>
                        <div class="flex justify-between"><span>ADDR</span><span>{cluster.best_node.host}</span></div>
                    </div>
                </div>
            {/if}
        </aside>

        <main class="flex-1 overflow-y-auto pr-2 space-y-3">
            <h2 class="text-purple-700 text-[10px] font-bold tracking-widest border-b border-purple-900/30 pb-1">WASM_REGISTRY</h2>
            {#each wasmCells as cell}
                <div class="bg-[#050505] border border-purple-900/20 p-4 hover:border-purple-500/40 transition-all flex justify-between items-center">
                    <div>
                        <div class="text-purple-300 font-bold text-xs">{cell.name}</div>
                        <div class="text-[9px] text-gray-600 mt-1">{cell.hit_count} DISPATCHES</div>
                    </div>
                    <button onclick={() => executeCell(cell.name)} class="px-3 py-1 border border-purple-600 text-purple-400 text-[9px] hover:bg-purple-600 hover:text-black">
                        ▶ DISPATCH
                    </button>
                </div>
            {:else}
                <div class="text-center py-10 text-gray-700 text-[10px] italic">Registry empty. Synthesis required.</div>
            {/each}
        </main>
    </div>
</div>
