<script>
    import { uplink } from '$lib/services/uplink.js';
    import { onMount } from 'svelte';

    let { activeFile = $bindable() } = $props();

    let currentPath = $state('.');
    let files = $state([]);
    let loading = $state(false);

    onMount(() => { loadDirectory('.'); });

    async function loadDirectory(targetPath) {
        loading = true;
        currentPath = targetPath;
        try {
            const res = await uplink.fetch(`/api/fs/list?path=${encodeURIComponent(targetPath)}`);
            if (res.ok) {
                let data = await res.json();
                data.sort((a, b) => {
                    if (a.is_dir === b.is_dir) return a.name.localeCompare(b.name);
                    return a.is_dir ? -1 : 1;
                });
                files = data;
            }
        } catch (e) { console.error("FS_EXPLORER_FAULT:", e); } 
        finally { loading = false; }
    }

    function navigateUp() {
        if (currentPath === '.') return;
        const parts = currentPath.split('/');
        parts.pop();
        loadDirectory(parts.length > 0 ? parts.join('/') : '.');
    }
</script>

<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded shadow-lg font-mono text-sm text-gray-300">
    <div class="flex items-center justify-between px-3 py-2 bg-[#161b22] border-b border-gray-800 shadow-sm">
        <span class="text-cyan-500 text-xs font-semibold uppercase tracking-wider">DNA_INDEX // {currentPath}</span>
        {#if currentPath !== '.'}
            <button onclick={navigateUp} class="text-xs text-gray-500 hover:text-cyan-400">[UP]</button>
        {/if}
    </div>
    <div class="flex-1 overflow-y-auto p-2">
        {#if loading}
            <div class="text-gray-600 italic px-2">Scanning substrate...</div>
        {:else if files.length === 0}
            <div class="text-gray-600 italic px-2">Directory empty.</div>
        {:else}
            {#each files as file}
                <button 
                    class="w-full text-left px-2 py-1 flex items-center gap-2 hover:bg-gray-800/50 hover:text-cyan-300 rounded transition-colors group"
                    onclick={() => file.is_dir ? loadDirectory(file.path) : (activeFile = file.path)}
                >
                    <span class={file.is_dir ? "text-amber-500/80" : "text-gray-500"}>
                        {file.is_dir ? '📁' : '📄'}
                    </span>
                    <span class="truncate">{file.name}</span>
                </button>
            {/each}
        {/if}
    </div>
</div>
