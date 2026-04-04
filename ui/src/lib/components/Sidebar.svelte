<script>
    import { onMount } from 'svelte';
    import { activeTab, openTabs, fileTreeTrigger } from '$lib/store';
    import { pushTermLine } from '$lib/components/NexusWS.js';

    let files = [];
    let newFileName = '';
    let creating = false;
    let error = '';

    async function loadFiles() {
        try {
            const res = await fetch('/api/fs/list');
            if (res.ok) {
                files = await res.json();
            }
        } catch (e) {
            pushTermLine('[FS] Index scan failed.', 'ansi-red');
        }
    }

    async function selectFile(path) {
        if (!path || path.endsWith('/')) return;
        // Already open?
        const existing = $openTabs.find(t => t.path === path);
        if (existing) { 
            activeTab.set(path); 
            return; 
        }
        
        try {
            const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
            if (!res.ok) throw new Error('read failed');
            const data = await res.json();
            openTabs.update(tabs => [...tabs, { path, content: data.content, isDirty: false }]);
            activeTab.set(path);
        } catch (err) {
            pushTermLine(`[FS] Could not open ${path}`, 'ansi-red');
        }
    }

    async function createFile() {
        const p = newFileName.trim();
        if (!p) return;
        error = '';
        try {
            const res = await fetch('/api/fs/write', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: p, content: '' }),
            });
            if (!res.ok) throw new Error(await res.text());
            newFileName = '';
            creating = false;
            await loadFiles();
            // Automatically open the new file
            await selectFile(p);
            pushTermLine(`[FS] Created ${p}`, 'ansi-green');
        } catch (e) {
            error = e.message;
        }
    }

    async function deleteFile(path, e) {
        e.stopPropagation();
        if (!confirm(`Delete ${path}?`)) return;
        try {
            const res = await fetch(`/api/fs/delete?path=${encodeURIComponent(path)}`, { method: 'DELETE' });
            if (!res.ok) throw new Error(await res.text());
            openTabs.update(tabs => tabs.filter(t => t.path !== path));
            if ($activeTab === path) activeTab.set('');
            await loadFiles();
            pushTermLine(`[FS] Deleted ${path}`, 'ansi-yellow');
        } catch (e) {
            pushTermLine(`[FS] Delete failed: ${e.message}`, 'ansi-red');
        }
    }

    onMount(loadFiles);
    $: if ($fileTreeTrigger) loadFiles();
</script>

<div class="h-full bg-[#0d1117] flex flex-col font-mono text-[10px]">
    <!-- Header -->
    <div class="px-3 py-2 border-b border-cyan-900/30 flex justify-between items-center shrink-0">
        <span class="text-[9px] text-gray-500 tracking-widest uppercase">Sector_Index</span>
        <button
            on:click={() => creating = !creating}
            class="text-cyan-700 hover:text-cyan-400 text-base leading-none transition-colors"
            title="New file">+</button>
    </div>

    <!-- New file input -->
    {#if creating}
        <div class="px-2 py-2 border-b border-cyan-900/20 bg-black/40 shrink-0">
            <input
                bind:value={newFileName}
                placeholder="src/new_file.rs"
                class="w-full bg-transparent border border-cyan-800/50 text-cyan-300 px-2 py-1 text-[9px] outline-none focus:border-cyan-500"
                on:keydown={(e) => { if (e.key === 'Enter') createFile(); if (e.key === 'Escape') creating = false; }}
                autofocus
            />
            {#if error}
                <p class="text-red-500 text-[8px] mt-1">{error}</p>
            {/if}
        </div>
    {/if}

    <!-- File list -->
    <div class="flex-1 overflow-y-auto p-2 space-y-0.5 custom-scrollbar">
        {#each files as file}
            <div
                role="button"
                tabindex="0"
                on:click={() => file.is_dir ? null : selectFile(file.path)}
                on:keydown={(e) => e.key === 'Enter' && !file.is_dir && selectFile(file.path)}
                class="w-full text-left px-2 py-1.5 rounded transition-all flex items-center gap-2 group cursor-pointer
                       {file.path === $activeTab
                           ? 'bg-cyan-900/30 text-cyan-400'
                           : 'text-gray-500 hover:bg-zinc-900 hover:text-gray-300'}"
            >
                <span class="shrink-0">{file.is_dir ? '📁' : '📄'}</span>
                <span class="truncate flex-1">{file.name}</span>
                
                {#if !file.is_dir}
                    <!-- dirty indicator from openTabs -->
                    {#if $openTabs.find(t => t.path === file.path)?.isDirty}
                        <span class="w-1.5 h-1.5 rounded-full bg-yellow-500 shrink-0"></span>
                    {/if}
                    
                    <button
                        on:click={(e) => deleteFile(file.path, e)}
                        class="opacity-0 group-hover:opacity-100 text-red-800 hover:text-red-500 px-1 transition-opacity"
                        title="Delete">✕</button>
                {/if}
            </div>
        {/each}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar       { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e293b; }
</style>
