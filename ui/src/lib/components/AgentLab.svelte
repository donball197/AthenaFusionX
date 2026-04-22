<script>
    import { onMount, onDestroy } from 'svelte';
    import { fly, fade } from 'svelte/transition';
    import { TaskService } from '$lib/services/TaskService.js';

    let cluster = $state({ best_node: null, has_capacity: false });
    let wasmCells = $state([]);
    let loading = $state(true);
    let pollTimer;

    async function fetchMeshState() {
        if (document.visibilityState === 'hidden') return;
        try {
            const [statusRes, cellsRes] = await Promise.all([
                fetch('/api/cluster/status'),
                fetch('/api/cluster/cells')
            ]);
            
            if (statusRes.ok) cluster = await statusRes.json();
            if (cellsRes.ok) {
                const data = await cellsRes.json();
                wasmCells = data.cells || [];
            }
        } catch (e) {
            console.warn("[MESH] Telemetry severed.", e);
        } finally {
            loading = false;
        }
    }

    onMount(() => {
        fetchMeshState();
        pollTimer = setInterval(fetchMeshState, 2000);
    });

    onDestroy(() => clearInterval(pollTimer));

    function executeCell(funcName, b64) {
        TaskService.log(`>> [MESH] Forcing execution of ${funcName} across cluster...`, 'text-cyan-400');
        fetch('/api/task', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ 
                directive: `Execute the WASM cell ${funcName} on the distributed mesh using wasm_b64="${b64}" and input=10.0. Halt when complete.` 
            })
        });
    }
</script>

<div class="h-full bg-black font-mono flex flex-col overflow-hidden p-6 gap-6">
    <header class="flex justify-between items-end border-b border-cyan-500/20 pb-4 flex-none">
        <div>
            <h1 class="text-cyan-400 text-xl font-black tracking-[4px] uppercase italic text-shadow-glow">Distributed_Mesh_Topology</h1>
            <p class="text-cyan-900 text-[10px] mt-1">PEER-TO-PEER ARM64 WASM ROUTING</p>
        </div>
        <div class="flex items-center gap-3 text-[10px]">
            {#if loading}
                <span class="text-amber-500 animate-pulse">SCANNING SUBSTRATE...</span>
            {:else if cluster.has_capacity}
                <span class="text-emerald-400 flex items-center gap-2">
                    <span class="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_8px_#34d399] animate-pulse"></span>
                    MESH ONLINE
                </span>
            {:else}
                <span class="text-rose-500 flex items-center gap-2">
                    <span class="w-2 h-2 rounded-full bg-rose-500 shadow-[0_0_8px_#f43f5e]"></span>
                    NO CAPACITY
                </span>
            {/if}
        </div>
    </header>

    <div class="flex-1 flex gap-6 min-h-0">
        <div class="w-1/3 flex flex-col gap-4">
            <h2 class="text-cyan-600 text-[10px] font-bold tracking-widest uppercase border-b border-cyan-900/30 pb-2">Active Routing Target</h2>
            
            {#if cluster.best_node}
                <div class="bg-cyan-950/10 border border-cyan-500/30 p-5 relative overflow-hidden" in:fly={{y: 10}}>
                    <div class="absolute top-0 right-0 p-2 text-cyan-500/20 text-4xl">◈</div>
                    <div class="text-cyan-300 font-bold tracking-wider mb-4">{cluster.best_node.id}</div>
                    
                    <div class="space-y-4 text-[10px]">
                        <div>
                            <div class="flex justify-between text-cyan-600 mb-1">
                                <span>CPU_LOAD</span>
                                <span>{cluster.best_node.cpu.toFixed(1)}%</span>
                            </div>
                            <div class="h-1 bg-black border border-cyan-900 overflow-hidden">
                                <div class="h-full bg-cyan-400 transition-all duration-500" style="width: {cluster.best_node.cpu}%"></div>
                            </div>
                        </div>
                        
                        <div>
                            <div class="flex justify-between text-purple-500 mb-1">
                                <span>RAM_RESIDENT</span>
                                <span>{cluster.best_node.ram.toFixed(1)}%</span>
                            </div>
                            <div class="h-1 bg-black border border-purple-900 overflow-hidden">
                                <div class="h-full bg-purple-400 transition-all duration-500" style="width: {cluster.best_node.ram}%"></div>
                            </div>
                        </div>

                        <div class="pt-3 border-t border-cyan-900/30 flex justify-between text-gray-500">
                            <span>HOST_IP</span>
                            <span class="text-cyan-500">{cluster.best_node.host}:{cluster.best_node.port}</span>
                        </div>
                    </div>
                </div>
            {:else if !loading}
                <div class="p-8 border border-dashed border-rose-900/50 text-center text-rose-500 text-[10px] bg-rose-950/10">
                    ALL NODES UNREACHABLE OR OVERLOADED
                </div>
            {/if}
        </div>

        <div class="w-2/3 flex flex-col gap-4">
            <h2 class="text-purple-600 text-[10px] font-bold tracking-widest uppercase border-b border-purple-900/30 pb-2">Compiled Cell Registry</h2>
            
            <div class="flex-1 overflow-y-auto custom-scrollbar pr-2 space-y-3">
                {#each wasmCells as cell (cell.name)}
                    <div class="bg-black border border-purple-900/30 p-4 hover:border-purple-500/50 transition-colors group" in:fly={{x: 20}}>
                        <div class="flex justify-between items-start mb-3">
                            <div>
                                <div class="text-purple-400 font-bold text-xs">{cell.name}</div>
                                <div class="text-gray-600 text-[9px] mt-1">{cell.created_at}</div>
                            </div>
                            <div class="text-right">
                                <div class="text-[10px] text-emerald-500 font-bold">{cell.hit_count} HITS</div>
                                <button 
                                    onclick={() => executeCell(cell.name, cell.wasm_b64)}
                                    class="mt-2 text-[8px] bg-purple-900/20 border border-purple-700 text-purple-300 px-3 py-1 hover:bg-purple-500 hover:text-black transition-all cursor-pointer">
                                    ▶ DISPATCH
                                </button>
                            </div>
                        </div>
                        <div class="bg-[#050505] border border-gray-900 p-3 text-[9px] text-gray-400 whitespace-pre-wrap overflow-x-auto">
                            {cell.source_code}
                        </div>
                    </div>
                {:else}
                    {#if !loading}
                        <div class="p-8 text-center text-gray-600 text-[10px] italic border border-gray-900">
                            Registry is empty. Use synthesize_wasm to forge a cell.
                        </div>
                    {/if}
                {/each}
            </div>
        </div>
    </div>
</div>

<style>
    .text-shadow-glow { text-shadow: 0 0 15px rgba(34, 211, 238, 0.4); }
    .custom-scrollbar::-webkit-scrollbar { width: 4px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: rgba(167, 139, 250, 0.3); border-radius: 2px; }
</style>
