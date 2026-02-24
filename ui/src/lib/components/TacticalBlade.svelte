<script>
    import { createEventDispatcher } from 'svelte';
    const dispatch = createEventDispatcher();

    export let open = false;
    export let onFileSelect;
    export let onToggleVim;

    let currentDirFiles = [];
    let globalIndex = [];
    let searchTerm = "";
    let selectedIndex = 0;

    async function createFile() {
        if (!searchTerm.trim()) return;
        const path = searchTerm.trim();
        const content = path.endsWith('.rs') ? '// Rust module\nfn main() {\n    println!("Hello, Nexus!");\n}' : '// New file';
        await fetch('/api/fs/write', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({path, content})
        });
        await buildGlobalIndex();
        onFileSelect(path); // Auto-open in tab
        if (path.endsWith('.rs')) {
            dispatch('build', {path}); // Auto-build new Rust files
        }
        open = false;
    }

    const systemCommands = [
        { name: 'Reload System', action: () => location.reload(), icon: '🔄', color: 'text-green-400' },
        { name: 'Toggle Vim Mode', action: () => { if(onToggleVim) onToggleVim(); }, icon: '🎮', color: 'text-yellow-400' },
        { name: 'Create File: ' + (searchTerm || '...'), action: createFile, icon: '+', color: 'text-white bg-green-600 px-1 rounded' }
    ];

    function getFileMeta(name, is_dir) {
        if (is_dir) return { icon: '📂', color: 'text-blue-400' };
        if (name.endsWith('.rs')) return { icon: '🦀', color: 'text-orange-400' };
        return { icon: '📄', color: 'text-gray-500' };
    }

    async function buildGlobalIndex() {
        try {
            const res = await fetch(`/api/fs/list?path=.&recursive=true`);
            if (res.ok) {
                globalIndex = await res.json();
                fetchDirectory(".");
            }
        } catch (e) {}
    }

    async function fetchDirectory(path) {
        if (globalIndex.length > 0) {
            currentDirFiles = globalIndex.filter(f => f.path.startsWith(path === "." ? "" : path));
        }
    }

    $: if (open && globalIndex.length === 0) buildGlobalIndex();

    $: displayList = (() => {
        const term = searchTerm.trim();
        if (term.startsWith('>')) return systemCommands.filter(c => !c.name.startsWith('Create'));
        const exactMatch = globalIndex.find(f => f.path === term);
        let list = globalIndex.filter(f => f.path.includes(term)).slice(0, 50);
        if (term.length > 0 && !exactMatch) {
            return [systemCommands[2], ...list];
        }
        return list;
    })();

    async function handleSelect(item) {
        if (item.action) {
            await item.action();
            open = false;
            return;
        }
        onFileSelect(item.path);
        open = false;
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

<svelte:window on:keydown={(e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        open = !open;
        searchTerm = "";
    }
    if (e.key === 'Escape') open = false;
}} />

{#if open}
    <div class="fixed inset-0 bg-black/50 backdrop-blur-[2px] z-[999]" on:click={() => open = false}></div>
    <div class="absolute bg-[#050505] border border-blue-900/50 rounded-sm z-[1000] shadow-2xl flex flex-col font-mono" style="left: 400px; top: 80px; width: 320px; max-height: 480px;">
        <div class="p-2 bg-[#080808]">
            <input
                type="text"
                bind:value={searchTerm}
                on:keydown={handleKeyDown}
                placeholder="> CMD or File..."
                class="w-full bg-[#111] border border-blue-900/30 text-white px-2 py-1.5 rounded outline-none focus:border-blue-500"
                autoFocus
            />
        </div>
        <div class="overflow-y-auto p-1 max-h-[300px] custom-scrollbar">
            {#each displayList as item, i}
                {@const meta = item.action ? {icon: item.icon, color: item.color} : getFileMeta(item.name, item.is_dir)}
                <button
                    on:click={() => handleSelect(item)}
                    class="w-full text-left px-2 py-1.5 text-[10px] flex items-center gap-2 {i === selectedIndex ? 'bg-blue-900/30 text-white' : 'hover:bg-[#111] text-gray-400'}"
                >
                    <span class="{meta.color}">{item.is_back ? '⤴' : meta.icon}</span>
                    <span class="truncate">{item.name}</span>
                </button>
            {/each}
        </div>
    </div>
{/if}

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1f242c; }
</style>
