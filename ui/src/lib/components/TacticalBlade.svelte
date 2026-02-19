<script>
    import { onMount } from 'svelte';
    import SoundFx from '$lib/SoundFx.js';

    export let open = false;
    export let onFileSelect;

    let currentDirFiles = [];
    let globalIndex = [];
    let searchTerm = "";
    let selectedIndex = 0;
    let scrollContainer;
    let x = 400; let y = 80;
    let width = 320;
    let isDragging = false;
    let isResizing = false;

    // SYSTEM COMMANDS
    const systemCommands = [
        { name: 'Reload System', action: () => location.reload(), icon: '🔄', color: 'text-green-400' },
        { name: 'Flush Memories', action: async () => { await fetch('/api/memories/purge', { method: 'DELETE' }); location.reload(); }, icon: '🧠', color: 'text-red-400' },
        { name: 'Save State', action: async () => { await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: 'nexus.state', content: 'SAVED'}) }); }, icon: '💾', color: 'text-blue-400' }
    ];

    function getFileMeta(name, is_dir) {
        if (is_dir) return { icon: '📂', color: 'text-blue-400' };
        if (name.endsWith('.rs')) return { icon: '🦀', color: 'text-orange-400' };
        if (name.endsWith('.svelte')) return { icon: '⚡', color: 'text-yellow-400' };
        if (name.endsWith('.js') || name.endsWith('.ts')) return { icon: '📜', color: 'text-yellow-200' };
        if (name.endsWith('.toml') || name.endsWith('.json')) return { icon: '⚙️', color: 'text-gray-400' };
        return { icon: '📄', color: 'text-gray-500' };
    }

    async function buildGlobalIndex() {
        try {
            const res = await fetch(`/api/fs/list?path=.&recursive=true`);
            if (res.ok) {
                globalIndex = await res.json();
                fetchDirectory(".");
            }
        } catch (e) { console.error("Index Error", e); }
    }

    async function fetchDirectory(path) {
        if (globalIndex.length > 0) {
            const cleanPath = path === "./" ? "." : path;
            const parts = cleanPath.split('/');
            
            // Direct Children Logic
            currentDirFiles = globalIndex.filter(f => {
                const fParts = f.path.split('/');
                if (cleanPath === ".") return fParts.length === 1 || (fParts.length === 2 && fParts[0] === ".");
                return f.path.startsWith(cleanPath + "/") && f.path.split('/').length === parts.length + 1;
            });
            
            if (cleanPath !== "." && cleanPath !== "") {
                const parent = parts.slice(0, -1).join('/') || ".";
                currentDirFiles = [{ name: "..", path: parent, is_dir: true, is_back: true }, ...currentDirFiles];
            }
        }
    }

    $: if (open && globalIndex.length === 0) buildGlobalIndex();

    // --- OMNIBOX LOGIC ---
    $: displayList = (() => {
        const term = searchTerm.trim();
        if (term.startsWith('>')) {
            return systemCommands.filter(c => c.name.toLowerCase().includes(term.slice(1).trim().toLowerCase()));
        }
        if (!term || (term.endsWith('/') && !term.includes('src/'))) { 
             return currentDirFiles; 
        }
        return globalIndex
            .filter(f => {
                const lowerTerm = term.toLowerCase();
                const lowerPath = f.path.toLowerCase();
                const lowerName = f.name.toLowerCase();
                if (lowerTerm.includes('/') || lowerTerm.startsWith('.')) {
                    const normTerm = lowerTerm.replace(/^\.\//, '');
                    return lowerPath.includes(normTerm);
                }
                return lowerName.includes(lowerTerm);
            })
            .sort((a, b) => {
                const aExact = a.path === term || a.name === term;
                const bExact = b.path === term || b.name === term;
                return bExact - aExact;
            })
            .slice(0, 50);
    })();

    async function handleSelect(item) {
        if (item.action) { await item.action(); open = false; return; }
        if (item.is_dir) {
            const newPath = item.is_back ? item.path : item.path;
            searchTerm = newPath === "." ? "" : newPath + "/";
            fetchDirectory(newPath);
        } else {
            onFileSelect(item.path);
            open = false;
        }
    }

    // --- WINDOW MANAGEMENT ---
    function handleGlobalKey(e) {
        if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
            e.preventDefault();
            open = !open;
        }
        if (e.key === 'Escape' && open) {
            open = false;
        }
    }

    function handleKeyDown(e) {
        if (!open) return;
        const max = displayList.length;
        if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = (selectedIndex + 1) % max; }
        else if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = (selectedIndex - 1 + max) % max; }
        else if (e.key === 'Enter') { 
            e.preventDefault(); 
            if (displayList[selectedIndex]) handleSelect(displayList[selectedIndex]); 
        }
    }
</script>

<svelte:window on:keydown={handleGlobalKey} 
               on:mousemove={(e) => isDragging && (x += e.movementX, y += e.movementY)} 
               on:mouseup={() => isDragging = false} />

{#if open}
    <div class="fixed inset-0 bg-black/20 backdrop-blur-[1px] z-[999]" on:click={() => open = false}></div>

    <div class="absolute bg-[#050505] border border-blue-900/50 rounded-sm z-[1000] shadow-[0_10px_40px_rgba(0,0,0,0.9)] flex flex-col font-mono transition-all duration-100 ease-out"
         style="left: {x}px; top: {y}px; width: {width}px; max-height: 480px;">
        
        <div class="h-6 bg-[#0a0a0a] border-b border-blue-900/30 flex justify-between items-center px-2 cursor-move"
             on:mousedown={() => isDragging = true}>
            <span class="text-[9px] text-blue-500 font-bold uppercase tracking-widest">
                NEXUS // {searchTerm.startsWith('>') ? 'SYS_CMD' : 'OMNIBOX'}
            </span>
            <button on:click={() => open = false} class="text-blue-800 hover:text-red-500 text-[10px]">✕</button>
        </div>

        <div class="p-2 bg-[#080808]">
            <input type="text" 
                   bind:value={searchTerm} 
                   on:keydown={handleKeyDown}
                   placeholder="> CMD or search..." 
                   autofocus
                   class="w-full bg-[#111] border border-blue-900/30 text-blue-100 text-[10px] px-2 py-1.5 rounded outline-none focus:border-blue-500 transition-colors" />
        </div>

        <div class="overflow-y-auto p-1 min-h-[50px] custom-scrollbar" bind:this={scrollContainer}>
            {#each displayList as item, i}
                {@const meta = item.action ? {icon: item.icon, color: item.color} : getFileMeta(item.name, item.is_dir)}
                <button on:click={() => handleSelect(item)}
                        class="w-full text-left px-2 py-1.5 text-[10px] flex items-center gap-2 {i === selectedIndex ? 'bg-blue-900/30 text-white' : 'hover:bg-[#111] text-gray-400'}">
                    <span class="{meta.color}">{item.is_back ? '⤴' : meta.icon}</span>
                    <span class="truncate">
                        {#if searchTerm.includes('/') && !item.action}
                             {item.path}
                        {:else}
                             {item.name}
                        {/if}
                    </span>
                </button>
            {/each}
            {#if displayList.length === 0}
                <div class="px-2 py-2 text-[10px] text-gray-600 text-center italic">NO SIGNAL</div>
            {/if}
        </div>
        
        <div class="absolute bottom-0 right-0 w-4 h-4 cursor-ew-resize flex items-end justify-end p-1"
             on:mousedown|stopPropagation={() => isResizing = true}>
             <div class="w-1.5 h-1.5 border-r border-b border-blue-800/50"></div>
        </div>
    </div>
{/if}

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e3a8a; }
    .custom-scrollbar::-webkit-scrollbar-track { background: #050505; }
</style>
