<script>
    import { terminalState } from '$lib/core/store.svelte';
    import { tick } from 'svelte';

    let logContainer = $state(null);
    let ghostContainer = $state(null);

    // Auto-scroll logic
    $effect(() => {
        let _l = terminalState.logs.length;
        if (logContainer) tick().then(() => logContainer.scrollTop = logContainer.scrollHeight);
    });
    $effect(() => {
        let _g = terminalState.ghostBuffer;
        if (ghostContainer) tick().then(() => ghostContainer.scrollTop = ghostContainer.scrollHeight);
    });
</script>

<div class="h-full flex p-4 border-t border-cyan-900/30">
    <div class="w-1/2 pr-4 border-r border-cyan-900/30 flex flex-col min-h-0">
        <h2 class="text-purple-500 font-black mb-2 text-[9px] tracking-[0.2em] flex-none">MATRIARCH_THOUGHTS</h2>
        <div bind:this={ghostContainer} class="flex-1 overflow-y-auto font-mono text-[10px] text-purple-300 whitespace-pre-wrap pr-2">
            {terminalState.ghostBuffer}
            {#if terminalState.isGhostTyping}<span class="animate-pulse">_</span>{/if}
        </div>
    </div>
    
    <div class="w-1/2 pl-4 flex flex-col min-h-0">
        <h2 class="text-emerald-500 font-black mb-2 text-[9px] tracking-[0.2em] flex-none">EXECUTION_LOG</h2>
        <div bind:this={logContainer} class="flex-1 overflow-y-auto font-mono text-[10px] space-y-1 pr-2">
            {#each terminalState.logs as log (log.id)}
                <div class="{log.cls}">
                    <span class="opacity-40 mr-2 shrink-0">[{log.timestamp}]</span>
                    <span class="break-words">{log.message}</span>
                </div>
            {/each}
        </div>
    </div>
</div>

<style>
    /* Sleek custom scrollbars */
    div::-webkit-scrollbar { width: 4px; }
    div::-webkit-scrollbar-thumb { background: rgba(0, 255, 255, 0.1); border-radius: 2px; }
    div::-webkit-scrollbar-thumb:hover { background: rgba(0, 255, 255, 0.3); }
</style>
