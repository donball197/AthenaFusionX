<script>
    import { createEventDispatcher } from 'svelte';
    const dispatch = createEventDispatcher();
    
    export let open = false;
    export let onFileSelect;
    export let onToggleVim; // Prop passed from parent

    let currentDirFiles = [];
    let globalIndex = [];
    let searchTerm = "";
    let selectedIndex = 0;
    let x = 400; let y = 80;
    
    // SYSTEM COMMANDS
    // Added 'Toggle Vim' here to bridge the logic
    const systemCommands = [
        { name: 'Reload System', action: () => location.reload(), icon: '🔄', color: 'text-green-400' },
        { name: 'Flush Memories', action: async () => { await fetch('/api/memories/purge', { method: 'DELETE' }); location.reload(); }, icon: '🧠', color: 'text-red-400' },
        { name: 'Toggle Vim Mode', action: () => { if(onToggleVim) onToggleVim(); }, icon: '🎮', color: 'text-yellow-400' },
        { name: 'Save State', action: async () => { await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: 'nexus.state', content: 'SAVED'}) }); }, icon: '💾', color: 'text-blue-400' }
    ];

    function getFileMeta(name, is_dir) {
        if (is_dir) return { icon: '📂', color: 'text-blue-400' };
        if (name.endsWith('.rs')) return { icon: '🦀', color: 'text-orange-400' };
        if (name.endsWith('.svelte')) return { icon: '⚡', color: 'text-yellow-400' };
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
            
            // Filter Logic
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
        // Force system commands if > is typed OR if list is empty and user just opened it
        if (term.startsWith('>')) {
            return systemCommands.filter(c => c.name.toLowerCase().includes(term.slice(1).trim().toLowerCase()));
        }
        if (!term) return currentDirFiles;
        
        return globalIndex
            .filter(f => f.name.toLowerCase().includes(term.toLowerCase()))
            .slice(0, 50);
    })();

    async function handleSelect(item) {
        if (item.action) { 
            await item.action(); 
            open = false; 
            return; 
        }
        if (item.is_dir) {
            searchTerm = "";
            fetchDirectory(item.path);
        } else {
            onFileSelect(item.path);
            open = false;
        }
    }

    function handleGlobalKey(e) {
        if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
            e.preventDefault();
            open = !open;
            if (open) { 
                 searchTerm = ""; 
                 setTimeout(() => document.getElementById('blade-input')?.focus(), 50);
            }
        }
        if (e.key === 'Escape' && open) open = false;
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

<svelte:window on:keydown={handleGlobalKey} />

{#if open}
    <div class="fixed inset-0 bg-black/50 backdrop-blur-[2px] z-[999]" on:click={() => open = false}></div>

    <div class="absolute bg-[#050505] border border-blue-900/50 rounded-sm z-[1000] shadow-[0_10px_40px_rgba(0,0,0,0.9)] flex flex-col font-mono"
         style="left: {x}px; top: {y}px; width: 320px; max-height: 480px;">
        
        <div class="p-2 bg-[#080808]">
            <input type="text" 
                   id="blade-input"
                   bind:value={searchTerm} 
                   on:keydown={handleKeyDown}
                   placeholder="> CMD or Filename..." 
                   autocomplete="off"
                   class="w-full bg-[#111] border border-blue-900/30 text-blue-100 text-[10px] px-2 py-1.5 rounded outline-none focus:border-blue-500 transition-colors" />
        </div>

        <div class="overflow-y-auto p-1 max-h-[300px] custom-scrollbar">
            {#each displayList as item, i}
                {@const meta = item.action ? {icon: item.icon, color: item.color} : getFileMeta(item.name, item.is_dir)}
                <button on:click={() => handleSelect(item)}
                        class="w-full text-left px-2 py-1.5 text-[10px] flex items-center gap-2 {i === selectedIndex ? 'bg-blue-900/30 text-white' : 'hover:bg-[#111] text-gray-400'}">
                    <span class="{meta.color}">{item.is_back ? '⤴' : meta.icon}</span>
                    <span class="truncate">{item.name}</span>
                </button>
            {/each}
        </div>
    </div>
{/if}

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e3a8a; }
</style>
