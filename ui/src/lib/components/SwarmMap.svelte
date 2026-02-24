<script>
    import { createEventDispatcher, onMount } from 'svelte';
    const dispatch = createEventDispatcher();
    export let resonance = 0.0;
    
    let nodes = [];
    
    // Mock Data for Visuals (Until /api/graph is live)
    // In production, this fetches from KnowledgeBroker
    onMount(async () => {
        try {
            const res = await fetch('/api/graph');
            if (res.ok) {
                const data = await res.json();
                nodes = data.nodes;
            }
        } catch(e) {}
    });
</script>

<div class="h-64 border border-[#30363d] bg-[#050505] relative overflow-hidden rounded">
    <div class="absolute top-2 left-2 text-[10px] text-gray-500 font-bold uppercase tracking-widest">
        Neural Graph [Resonance: {resonance.toFixed(2)}]
    </div>
    
    {#if nodes.length === 0}
        <div class="flex items-center justify-center h-full text-gray-700 text-xs italic">
            Scanning Neural Architecture...
        </div>
    {:else}
        <!-- Simple Visualization Logic -->
        <div class="p-4 grid grid-cols-4 gap-2">
            {#each nodes as node}
                <div class="w-2 h-2 rounded-full bg-blue-500/50" title={node.id}></div>
            {/each}
        </div>
    {/if}
</div>
