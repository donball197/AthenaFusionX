<script>
    import { onMount } from 'svelte';
    import { ide, fileTreeTrigger, layout } from '$lib/core/store.svelte.js';
    import { listFiles, readFile } from '$lib/api.js';

    let files = $state([]);
    let loading = $state(false);

    async function refresh() {
        loading = true;
        const res = await listFiles(".");
        files = Array.isArray(res) ? res : [];
        loading = false;
    }

    onMount(refresh);

    $effect(() => {
        // Trigger via Svelte 5 object property access
        if (fileTreeTrigger.value >= 0) {
            refresh();
        }
    });

    async function openFile(path) {
        if (!path) return;
        const data = await readFile(path);
        if (data && data.content) {
            ide.mountFile(path, data.content);
        }
    }
</script>

<div class="h-full flex flex-col bg-[#0a0a0a] font-mono text-xs border-r border-cyan-900/30">
    <div class="px-4 py-3 border-b border-cyan-900/30 flex justify-between items-center bg-black/60">
        <span class="text-cyan-400 font-bold tracking-widest uppercase text-[10px]">DNA_FS</span>
        <button onclick={refresh} class="text-cyan-600 hover:text-cyan-400 transition-colors">⟳</button>
    </div>

    <div class="flex-1 overflow-y-auto p-2 custom-scrollbar">
        {#if loading && files.length === 0}
            <div class="text-center py-8 text-cyan-800 animate-pulse">Scanning DNA...</div>
        {:else}
            {#each files as file}
                <button
                    onclick={() => file.is_dir ? null : openFile(file.path)}
                    class="w-full text-left px-3 py-1.5 rounded flex items-center gap-2 hover:bg-cyan-900/20 transition-all group
                           {ide.activeTab === file.path ? 'bg-cyan-900/40 text-cyan-300' : 'text-gray-500 hover:text-gray-300'}">
                    <span class="opacity-50 group-hover:opacity-100">{file.is_dir ? '📁' : '📄'}</span>
                    <span class="truncate">{file.name}</span>
                </button>
            {/each}
        {/if}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1f2937; }
</style>
