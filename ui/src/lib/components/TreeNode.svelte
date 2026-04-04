<script>
    export let node;
    export let level = 0;
</script>

<div class="pl-{level * 4} text-xs">
    <div class="flex items-center gap-2 py-1 text-gray-300 hover:text-white">
        {#if node.children}
            <span class="text-green-600">▾</span>
        {:else}
            <span class="text-gray-600">→</span>
        {/if}
        <span>{node.name}</span>
    </div>
</div>
