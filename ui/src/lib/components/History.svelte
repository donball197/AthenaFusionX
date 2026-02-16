<script>
    import SoundFx from '$lib/SoundFx.js';
    export let memories = [];
    export let onReplay;
    export let onRefresh;

    async function purge() {
        if (!confirm("WIPE ALL NEURAL MEMORIES?")) return;
        SoundFx.click();
        await fetch('/api/memories/purge', { method: 'DELETE' });
        onRefresh();
    }

    async function exportMem() {
        SoundFx.click();
        const res = await fetch('/api/memories/archive', { method: 'POST' });
        if (res.ok) alert("ARCHIVE SECURED ON DISK");
    }
</script>

<div class="flex flex-col gap-2">
    <div class="text-[10px] text-[#58a6ff] border-b border-[#30363d] flex justify-between uppercase mb-2 tracking-widest font-bold">
        <span>Memories</span>
        <div class="flex gap-2">
            <button on:click={exportMem} class="text-[8px] text-gray-500 hover:text-green-400 uppercase">Export</button>
            <button on:click={purge} class="text-[8px] text-gray-500 hover:text-red-400 uppercase">Purge</button>
        </div>
    </div>
    <div class="flex flex-col gap-2 max-h-48 overflow-y-auto custom-scrollbar">
        {#each memories.slice().reverse() as mem}
            <button on:click={() => onReplay(mem)} class="text-left p-2 border border-[#30363d] bg-black rounded hover:border-[#58a6ff] transition-all group">
                <div class="text-[9px] text-gray-500 italic line-clamp-1 group-hover:text-white transition-colors">"{mem.prompt}"</div>
            </button>
        {/each}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1f242c; }
</style>
