<script>
    let { response = "", active = false } = $props();
</script>

<div class="flex flex-col h-full bg-[#050505] border-l border-cyan-900/30 font-mono">
    <div class="px-4 py-2 bg-cyan-900/10 border-b border-cyan-900/30 flex items-center justify-between">
        <span class="text-[10px] font-black tracking-widest text-cyan-400">GHOST_CORTEX</span>
        {#if active}
            <span class="text-[9px] text-cyan-500 animate-pulse">● LIVE STREAM</span>
        {/if}
    </div>

    <div class="flex-1 p-4 overflow-y-auto text-[11px] text-cyan-100 whitespace-pre-wrap leading-relaxed custom-scrollbar">
        {#if active && !response}
            <span class="animate-pulse text-cyan-700">>> AWAITING NEURAL UPLINK...</span>
        {:else}
            {response || ">> UPLINK STABLE. AWAITING DIRECTIVE..."}
        {/if}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1f2937; }
</style>
