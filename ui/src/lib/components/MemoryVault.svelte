<script>
    import { onMount, onDestroy } from 'svelte';

    let memories = $state([]);

    async function refresh() {
        try {
            const res = await fetch('/api/memories');
            if (res.ok) memories = await res.json();
        } catch (e) {
            console.error('[MemoryVault] fetch error:', e);
        }
    }

    // FIX: Use onMount/onDestroy so the listener is properly cleaned up
    // when the component is unmounted — the bare window.addEventListener
    // at module level leaked one handler every time the component mounted.
    onMount(() => {
        refresh();
        window.addEventListener('nexus:task-complete', refresh);
    });

    onDestroy(() => {
        window.removeEventListener('nexus:task-complete', refresh);
    });
</script>

<div class="h-full flex flex-col bg-[#050505] border border-purple-900/30 font-mono text-[10px]">
    <div class="p-2 border-b border-purple-900/30 flex justify-between items-center bg-[#0a0510]">
        <span class="text-purple-400 font-bold uppercase tracking-widest">Memory Palace</span>
        <button onclick={refresh} class="text-purple-600 hover:text-purple-400">⟳</button>
    </div>

    <div class="flex-1 overflow-y-auto p-2 space-y-2 custom-scrollbar">
        {#each memories as mem}
            <div class="p-2 border border-purple-900/20 bg-purple-950/5 rounded group">
                <div class="flex justify-between text-purple-500/50 mb-1">
                    <span>{mem.origin}</span>
                    <span>{mem.timestamp}</span>
                </div>
                <!--
                    Safe text interpolation — directive and result come from
                    the database (which stores LLM output) so @html is unsafe.
                -->
                <div class="text-gray-300 group-hover:text-purple-300 transition-colors truncate">
                    {mem.directive}
                </div>
                <div class="text-gray-600 text-[9px] truncate mt-1 italic">
                    {mem.result?.slice(0, 120) || ''}
                </div>
            </div>
        {/each}
        {#if memories.length === 0}
            <div class="text-center py-8 text-gray-700 italic">No persistent memories established.</div>
        {/if}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar       { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #2d1b69; }
</style>
