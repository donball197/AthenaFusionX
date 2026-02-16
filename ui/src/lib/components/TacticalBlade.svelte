<script>
    import { onMount } from 'svelte';
    import SoundFx from '$lib/SoundFx.js';

    export let open = false;
    export let onFileSelect;
    
    // DATA SOURCES
    let currentDirFiles = [];
    let globalIndex = [];
    
    // SYSTEM COMMANDS
    const systemCommands = [
        { name: 'Reload System', action: () => location.reload(), icon: '🔄', color: 'text-green-400' },
        { name: 'Flush Memories', action: async () => { await fetch('/api/memories/purge', { method: 'DELETE' }); location.reload(); }, icon: '🧠', color: 'text-red-400' },
        { name: 'Save State', action: async () => { await fetch('/api/fs/write', { method: 'POST', body: JSON.stringify({path: 'nexus.state', content: 'SAVED'}) }); }, icon: '💾', color: 'text-blue-400' }
    ];

    let searchTerm = "";
    let selectedIndex = 0;
    let scrollContainer;
    
    let x = 400; let y = 80;
    let width = 320; 
    let isDragging = false;
    let isResizing = false;

    function getFileMeta(name, is_dir) {
        if (is_dir) return { icon: '📂', color: 'text-blue-400' };
        if (name.endsWith('.rs')) return { icon: '🦀', color: 'text-orange-400' };
        if (name.endsWith('.svelte')) return { icon: '⚡', color: 'text-yellow-400' };
        if (name.endsWith('.js') || name.endsWith('.ts')) return { icon: '📜', color: 'text-yellow-200' };
        if (name.endsWith('.toml') || name.endsWith('.json')) return { icon: '⚙️', color: 'text-gray-400' };
        if (name.endsWith('.lock')) return { icon: '🔒', color: 'text-red-400' };
        if (name.endsWith('.md')) return { icon: '📝', color: 'text-purple-400' };
        if (name.endsWith('.sh')) return { icon: '💻', color: 'text-green-400' };
        return { icon: '📄', color: 'text-gray-500' };
    }

    async function buildGlobalIndex(path = ".", recursive = true) {
        try {
            const res = await fetch(`/api/fs/list?path=${encodeURIComponent(path)}`);
            if (res.ok) {
                const data = await res.json();
                const newItems = data.filter(d => !globalIndex.find(g => g.path === d.path));
                globalIndex = [...globalIndex, ...newItems];
                if (recursive) {
                    for (const item of data) {
                        if (item.is_dir && !['node_modules', 'target', '.git'].some(bad => item.path.includes(bad))) {
                            await buildGlobalIndex(item.path, true);
                        }
                    }
                }
            }
        } catch (e) { console.error(e); }
    }

    async function fetchDirectory(path) {
        try {
            const apiPath = path.endsWith('/') && path !== "./" ? path.slice(0, -1) : path;
            const res = await fetch(`/api/fs/list?path=${encodeURIComponent(apiPath)}`);
            if (res.ok) {
                let data = await res.json();
                data.sort((a, b) => b.is_dir - a.is_dir || a.name.localeCompare(b.name));
                
                if (path !== "." && path !== "./" && path !== "") {
                    const parts = path.split('/').filter(p => p && p !== ".");
                    const parent = parts.length > 1 ? parts.slice(0, -1).join('/') + "/" : "./";
                    data = [{ name: "..", path: parent, is_dir: true, is_back: true }, ...data];
                }
                currentDirFiles = data;
                selectedIndex = 0;
            }
        } catch (e) { console.error(e); }
    }

    $: if (open) {
        if (globalIndex.length === 0) buildGlobalIndex();
        if (!searchTerm) fetchDirectory(".");
    }

    $: displayList = (() => {
        const term = searchTerm.trim();
        
        // COMMAND MODE
        if (term.startsWith('>')) {
            const cmd = term.slice(1).trim().toLowerCase();
            return systemCommands.filter(c => c.name.toLowerCase().includes(cmd));
        }

        // BROWSE & SEARCH (Existing Logic)
        if (!term || term.endsWith('/')) return currentDirFiles;
        if (term.includes('/')) {
            const lastSlash = term.lastIndexOf('/');
            const query = term.slice(lastSlash + 1).toLowerCase();
            return currentDirFiles.filter(f => f.name.toLowerCase().includes(query));
        }
        return globalIndex
            .filter(f => f.name.toLowerCase().includes(term.toLowerCase()))
            .map(f => {
                let score = 0;
                if (f.name === term) score += 100;
                if (f.name.startsWith(term)) score += 50;
                return { ...f, score };
            })
            .sort((a, b) => b.score - a.score)
            .slice(0, 50);
    })();

    $: if (selectedIndex !== undefined && scrollContainer) {
        const el = scrollContainer.querySelector(`[data-index="${selectedIndex}"]`);
        if (el) el.scrollIntoView({ block: 'nearest' });
    }

    async function handleSelect(item) {
        // COMMAND EXECUTION
        if (item.action) {
            SoundFx.processing();
            await item.action();
            open = false;
            return;
        }

        // FILE NAVIGATION
        if (item.is_dir) {
            SoundFx.click();
            let newPath = item.is_back ? item.path : (item.path.endsWith('/') ? item.path : item.path + "/");
            searchTerm = newPath;
            await fetchDirectory(newPath);
        } else {
            SoundFx.success();
            onFileSelect(item.path);
            open = false;
        }
    }

    function handleKeyDown(e) {
        if (!open) return;
        const max = displayList.length;
        if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = (selectedIndex + 1) % max; }
        else if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = (selectedIndex - 1 + max) % max; }
        else if (e.key === 'Enter') { e.preventDefault(); if (displayList[selectedIndex]) handleSelect(displayList[selectedIndex]); }
        else if (e.key === 'Tab') {
            e.preventDefault();
            if (displayList[selectedIndex] && !displayList[selectedIndex].action) {
                const file = displayList[selectedIndex];
                if (file.is_dir) handleSelect(file);
                else { searchTerm = file.path; SoundFx.click(); }
            }
        }
        else if (e.key === 'Escape') open = false;
    }
</script>

<svelte:window on:mousemove={(e) => { if(isDragging) { x += e.movementX; y += e.movementY; } if(isResizing) { width += e.movementX; } }} 
               on:mouseup={() => {isDragging = false; isResizing = false;}} />

{#if open}
<div class="absolute bg-[#050505] border border-blue-900/50 rounded-sm z-[1000] shadow-[0_0_50px_rgba(0,0,0,0.8)] flex flex-col font-mono"
     style="left: {x}px; top: {y}px; width: {width}px; max-height: 480px;">
    
    <div class="h-6 shrink-0 bg-[#0a0a0a] border-b border-blue-900/30 flex justify-between items-center px-2 cursor-move select-none"
         on:mousedown={() => isDragging = true}>
        <span class="text-[9px] text-blue-500 font-bold tracking-widest uppercase">
            NEXUS // {searchTerm.startsWith('>') ? 'COMMAND_MODE' : (searchTerm.includes('/') ? 'NAV' : 'FIND')}
        </span>
        <button on:click={() => open = false} class="text-blue-800 hover:text-red-500 text-[10px]">✕</button>
    </div>

    <div class="p-2 bg-[#080808] shrink-0">
        <input type="text" 
               bind:value={searchTerm} 
               on:keydown={handleKeyDown}
               on:input={() => { if(searchTerm.endsWith('/')) fetchDirectory(searchTerm); }}
               placeholder="> CMD or Navigate..."
               autofocus
               class="w-full bg-[#111] border border-blue-900/30 text-blue-100 text-[10px] px-2 py-1.5 rounded focus:border-blue-500 outline-none font-mono transition-all" />
    </div>
    
    <div class="overflow-y-auto custom-scrollbar p-1 min-h-[50px]" bind:this={scrollContainer}>
        {#each displayList as item, i}
            {@const meta = item.action ? {icon: item.icon, color: item.color} : getFileMeta(item.name, item.is_dir)}
            <button on:click={() => handleSelect(item)} 
                    data-index={i}
                    class="w-full text-left px-2 py-1.5 text-[10px] font-mono flex items-center justify-between group {i === selectedIndex ? 'bg-blue-900/20' : 'hover:bg-[#111]'}">
                
                <div class="flex items-center gap-2 overflow-hidden">
                    <span class="{meta.color} {item.is_back ? 'text-blue-200' : ''}">
                        {item.is_back ? '⤴' : meta.icon}
                    </span>
                    <span class="truncate {i === selectedIndex ? 'text-white' : 'text-gray-400'}">
                        {item.name}
                    </span>
                </div>

                {#if i === selectedIndex}
                    <span class="text-[7px] uppercase tracking-wider text-blue-500 font-bold animate-pulse">
                        {item.action ? 'EXEC' : (item.is_dir ? 'DRILL' : 'EDIT')}
                    </span>
                {/if}
            </button>
        {/each}
        
        {#if displayList.length === 0}
            <div class="text-center py-4 text-[9px] text-gray-700 italic">VOID</div>
        {/if}
    </div>

    <div class="absolute bottom-0 right-0 w-3 h-3 cursor-ew-resize flex items-end justify-end p-0.5"
         on:mousedown|stopPropagation={() => isResizing = true}>
        <div class="w-1 h-1 border-r border-b border-blue-800"></div>
    </div>
</div>
{/if}

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e3a8a; }
    .custom-scrollbar::-webkit-scrollbar-track { background: #050505; }
</style>
