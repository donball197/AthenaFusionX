<script>
    import SoundFx from '$lib/SoundFx.js';
    export let memories = [];
    export let onReplay;
    export let onRefresh;

    async function purge() {
        if (!confirm("WIPE ALL NEURAL MEMORIES?")) return;
        SoundFx.click?.();
        await fetch('/api/memories/purge', { method: 'DELETE' });
        onRefresh();
    }

    async function exportMem() {
        SoundFx.click?.();
        const res = await fetch('/api/memories/archive', { method: 'POST' });
        if (res.ok) alert("ARCHIVE SECURED");
    }
</script>

<div>
    <div class="text-[#58a6ff] text-xs border-b border-[#30363d] pb-2 mb-3 flex justify-between uppercase tracking-widest font-bold">
        <span>NEURAL MEMORIES</span>
        <div class="flex gap-4 text-[9px]">
            <button on:click={exportMem} class="hover:text-green-400">EXPORT</button>
            <button on:click={purge} class="hover:text-red-400">PURGE</button>
        </div>
    </div>

    <div class="max-h-44 overflow-y-auto custom-scrollbar space-y-2">
        {#each memories.slice().reverse() as mem}
            <button 
                on:click={() => onReplay(mem)}
                class="w-full text-left p-3 border border-[#30363d] hover:border-[#58a6ff] bg-black rounded transition-all group">
                <div class="text-xs text-gray-400 italic line-clamp-2 group-hover:text-gray-200">
                    "{mem.prompt}"
                </div>
            </button>
        {/each}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1f242c; }
</style>
