<script>
    import { onMount } from 'svelte';
    let memories = [];

    async function refresh() {
        const res = await fetch('/api/memories');
        if (res.ok) memories = await res.json();
    }

    onMount(refresh);
    
    // Listen for completion events from the terminal to refresh memories
    if (typeof window !== 'undefined') {
        window.addEventListener('nexus:task-complete', refresh);
    }
</script>

<div class="h-full flex flex-col bg-[#050505] border border-purple-900/30 font-mono text-[10px]">
    <div class="p-2 border-b border-purple-900/30 flex justify-between items-center bg-[#0a0510]">
        <span class="text-purple-400 font-bold uppercase tracking-widest">Memory Palace</span>
        <button on:click={refresh} class="text-purple-600 hover:text-purple-400">⟳</button>
    </div>
    <div class="flex-1 overflow-y-auto p-2 space-y-2 custom-scrollbar">
        {#each memories as mem}
            <div class="p-2 border border-purple-900/20 bg-purple-950/5 rounded group">
                <div class="flex justify-between text-purple-500/50 mb-1">
                    <span>{mem.category}</span>
                    <span>{new Date(mem.timestamp * 1000).toLocaleTimeString()}</span>
                </div>
                <div class="text-gray-300 group-hover:text-purple-300 transition-colors">
                    {mem.key}: <span class="text-gray-500 italic">{mem.value}</span>
                </div>
            </div>
        {/each}
        {#if memories.length === 0}
            <div class="text-center py-8 text-gray-700 italic">No persistent memories established.</div>
        {/if}
    </div>
</div>
