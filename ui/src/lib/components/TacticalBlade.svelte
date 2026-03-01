<script>
    import { onMount } from 'svelte';
    import TreeItem from './TreeItem.svelte';

    export let open = false;
    export let onFileSelect = () => {};
    export let onToggleVim = () => {};

    let files = [];
    let loading = false;

    async function fetchTree() {
        if (loading) return;
        loading = true;
        try {
            const res = await fetch('/api/fs/tree');
            if (res.ok) files = await res.json();
        } catch (e) { console.error("Tree Error", e); }
        finally { loading = false; }
    }

    $: if (open && files.length === 0) fetchTree();

    function handleSelect(e) {
        onFileSelect(e.detail);
    }
</script>

<aside 
    class="absolute left-0 top-0 bottom-0 bg-[#0d1117] border-r border-green-900/30 z-[100] transition-all duration-300 pointer-events-auto flex flex-col"
    class:w-64={open}
    class:w-0={!open}
    style="overflow-x: hidden;"
>
    <div class="p-3 border-b border-green-900/25 flex justify-between items-center shrink-0">
        <span class="text-[9px] tracking-wider uppercase font-bold text-green-600">TACTICAL BLADE</span>
        <div class="flex gap-2">
            <button on:click={fetchTree} class="text-[9px] text-gray-500 hover:text-white" title="Refresh">↻</button>
            <button on:click={onToggleVim} class="text-[9px] text-gray-500 hover:text-white" title="Vim Mode">VIM</button>
        </div>
    </div>

    <div class="flex-1 overflow-y-auto custom-scrollbar py-2">
        {#each files as file}
            <TreeItem entry={file} on:select={handleSelect} />
        {/each}
        {#if files.length === 0 && !loading}
             <div class="p-4 text-center text-gray-600 text-[9px] italic">Offline / No Files</div>
        {/if}
    </div>
</aside>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #064e3b; }
</style>
