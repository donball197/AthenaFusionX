<script>
    import { onMount } from 'svelte';
    let { resonance = 0.0 } = $props();

    let nodes = $state([]);

    onMount(async () => {
        try {
            const res = await fetch('/api/graph');
            if (res.ok) nodes = (await res.json()).nodes || [];
        } catch(e) {}
    });
</script>

<div class="h-52 border border-[#30363d] bg-[#050505] rounded relative overflow-hidden">
    <div class="absolute top-3 left-3 text-[10px] text-gray-500 tracking-widest">
        NEURAL GRAPH • RES: {resonance.toFixed(2)}
    </div>

    {#if nodes.length === 0}
        <div class="flex items-center justify-center h-full text-gray-600 text-xs italic">
            Scanning neural architecture...
        </div>
    {:else}
        <div class="absolute inset-0 flex items-center justify-center">
            <div class="grid grid-cols-5 gap-3 p-6">
                {#each nodes.slice(0, 15) as node}
                    <div class="w-3 h-3 rounded-full bg-blue-500/60 hover:bg-blue-400 transition-all" title={node.id}></div>
                {/each}
            </div>
        </div>
    {/if}
</div>
