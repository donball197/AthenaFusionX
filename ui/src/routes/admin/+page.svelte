<script>
    import MemoryVault from '$lib/components/MemoryVault.svelte';
</script>

<div class="h-full bg-black p-6 flex flex-col gap-4">
    <header class="flex justify-between items-center border-b border-purple-900/30 pb-4">
        <h1 class="text-purple-400 font-black tracking-[4px] uppercase italic">System_Administrator</h1>
        <span class="text-[10px] text-purple-900">LEVEL_0_ACCESS_GRANTED</span>
    </header>
    <div class="flex-1 border border-purple-900/20 bg-purple-950/5">
        <MemoryVault />
    </div>
</div>
