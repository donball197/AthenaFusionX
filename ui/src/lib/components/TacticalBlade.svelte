<script>
    import { createEventDispatcher } from 'svelte';
    const dispatch = createEventDispatcher();

    export let files = [];
    export let activePath = "";

    function handleSelect(path) {
        dispatch('select', { path });
    }
</script>

<div class="flex flex-col h-full bg-[#0d1117]">
    <div class="px-5 py-4 border-b border-[#30363d] text-xs text-gray-500 tracking-widest uppercase">
        SECTOR INDEX
    </div>
    <div class="flex-1 overflow-y-auto p-3 space-y-1 custom-scrollbar">
        {#each files as file}
            <button
                on:click={() => handleSelect(file.path)}
                class="w-full text-left px-4 py-3 rounded text-xs transition-all flex items-center gap-3
                {activePath === file.path 
                    ? 'bg-blue-900/30 text-blue-400 border-l-2 border-blue-500' 
                    : 'text-gray-400 hover:bg-zinc-900 hover:text-gray-200'}">
                <span>{file.is_dir ? '📁' : '📄'}</span>
                <span class="truncate">{file.name}</span>
            </button>
        {/each}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #27272a; }
</style>
