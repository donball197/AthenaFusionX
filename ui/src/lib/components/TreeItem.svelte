<script>
    import { createEventDispatcher } from 'svelte';
    const dispatch = createEventDispatcher();
    
    export let entry;
    export let depth = 0;
    
    let expanded = false;
    
    function toggle() {
        if (entry.is_dir) expanded = !expanded;
        else dispatch('select', entry.path);
    }
    
    function forward(e) {
        dispatch('select', e.detail);
    }
</script>

<div class="flex flex-col">
    <button 
        class="text-left py-0.5 px-2 hover:bg-green-900/20 flex items-center gap-1.5 transition-colors group w-full truncate"
        style="padding-left: {(depth * 12) + 8}px"
        on:click={toggle}
    >
        <span class="opacity-60 text-[9px] group-hover:text-white group-hover:opacity-100">
            {#if entry.is_dir}
                {expanded ? '📂' : '📁'}
            {:else}
                📄
            {/if}
        </span>
        <span class="text-[10px] truncate font-mono {entry.is_dir ? 'text-green-500 font-bold' : 'text-gray-400'}">
            {entry.name}
        </span>
    </button>

    {#if expanded && entry.children}
        {#each entry.children as child}
            <svelte:self entry={child} depth={depth + 1} on:select={forward} />
        {/each}
    {/if}
</div>
