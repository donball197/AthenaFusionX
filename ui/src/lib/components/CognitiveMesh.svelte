<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { cognitiveKernel, nodeRegistry, type KernelProcess } from "$lib/core/mesh_topology";
    import { pushTermLine } from "$lib/core/NexusWS.js";

    let activeCells: KernelProcess[] = $state([]);
    let wasmResult = $state<number | null>(null);

    // ⚡ V14: The V8 WASM Engine
    async function handleGenesis(event: CustomEvent<string>) {
        const base64 = event.detail;
        try {
            pushTermLine(">> [BROWSER_V8] Intercepted GENESIS payload. Instantiating WebAssembly...", "text-purple-400");
            const binaryString = window.atob(base64);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) bytes[i] = binaryString.charCodeAt(i);
            
            const { instance } = await WebAssembly.instantiate(bytes.buffer);
            
            if (typeof instance.exports.execute_cell === 'function') {
                const testInput = Math.random() * 100;
                // @ts-ignore
                const res = instance.exports.execute_cell(testInput);
                wasmResult = res;
                
                // Add the newly forged cell to the UI visualization
                cognitiveKernel.registry.set("dynamic_wasm", {
                    cell: { id: "dynamic_wasm", label: "Synthesized Logic Cell", synapses: ["v8_engine"] },
                    nodeId: "DUET_LOCAL",
                    state: "active"
                });
                
                pushTermLine(`>> [BROWSER_V8] Execution Success! In: ${testInput.toFixed(2)} | Out: ${res.toFixed(2)}`, "text-emerald-400");
            } else {
                pushTermLine("!! [BROWSER_V8] Error: execute_cell export missing.", "text-rose-400");
            }
        } catch (e: any) {
            pushTermLine(`!! [BROWSER_V8] Instantiation Fault: ${e.message}`, "text-rose-400");
        }
    }

    onMount(() => {
        window.addEventListener("nexus-genesis", handleGenesis as EventListener);
        
        nodeRegistry.register({ id: "DUET_LOCAL", type: "local", capacity: 100, load: 20, lastHeartbeat: Date.now() });
        cognitiveKernel.ignite("logic_audit");
        cognitiveKernel.ignite("thermal_monitor");

        const loop = setInterval(() => {
            nodeRegistry.register({ id: "DUET_LOCAL", type: "local", capacity: 100, load: Math.random()*40, lastHeartbeat: Date.now() });
            activeCells = cognitiveKernel.snapshot();
        }, 1000);
        return () => clearInterval(loop);
    });

    onDestroy(() => {
        if (typeof window !== 'undefined') window.removeEventListener("nexus-genesis", handleGenesis as EventListener);
    });
</script>

<div class="p-6 font-mono h-full bg-black overflow-y-auto">
    <div class="text-pink-400 font-bold mb-6 tracking-widest border-b border-pink-500/20 pb-2">NEXUS_COGNITIVE_KERNEL_v14</div>
    
    {#if wasmResult !== null}
        <div class="mb-6 p-4 border border-emerald-500/30 bg-emerald-950/10 text-emerald-400 shadow-[0_0_20px_rgba(52,211,153,0.2)]">
            <div class="text-[10px] font-bold mb-1 tracking-widest">DYNAMIC_WASM_OUTPUT</div>
            <div class="text-3xl font-black">{wasmResult.toFixed(4)}</div>
            <div class="text-[8px] text-emerald-700 mt-2">Executed natively in V8 Engine via compiled FSM payload</div>
        </div>
    {/if}

    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each activeCells as p}
            <div class="border {p.state === 'active' ? 'border-pink-500/50 shadow-[0_0_15px_rgba(244,114,182,0.15)]' : 'border-gray-800'} bg-pink-950/5 p-4 relative transition-all">
                <div class="flex justify-between text-[10px] mb-3">
                    <span class="text-pink-300 font-bold">{p.cell.label}</span>
                    <span class="text-emerald-400">{p.state.toUpperCase()}</span>
                </div>
                <div class="text-[9px] text-gray-500">NODE_AFFINITY: <span class="text-gray-300">{p.nodeId}</span></div>
                <div class="mt-2 text-[8px] text-purple-400">
                    {#each p.cell.synapses as s}<div>→ SYNAPSE: {s}</div>{/each}
                </div>
            </div>
        {/each}
    </div>
</div>
