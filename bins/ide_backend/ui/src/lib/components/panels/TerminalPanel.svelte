<script>
    /**
     * TerminalPanel.svelte — v2.0 [UNIFIED_STORE]
     * FIX: Removed redundant WebSocket connection. Now consumes the central 
     *      terminal store populated by runtime.ts.
     */
    import { terminal } from '$lib/stores/terminal.svelte';
    import { tick } from 'svelte';

    let terminalContainer = $state(null);

    // Auto-scroll logic using Svelte 5 Runes
    $effect(() => {
        // Track logs length for reactivity
        if (terminal.logs.length >= 0 && terminalContainer) {
            tick().then(() => {
                terminalContainer.scrollTop = terminalContainer.scrollHeight;
            });
        }
    });
</script>

<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded shadow-lg font-mono text-sm">
    <div class="flex items-center px-4 py-2 bg-[#161b22] border-b border-gray-800 shadow-sm">
        <div class="flex gap-1.5 mr-4">
            <div class="w-2.5 h-2.5 rounded-full bg-[#ff5f56]"></div>
            <div class="w-2.5 h-2.5 rounded-full bg-[#ffbd2e]"></div>
            <div class="w-2.5 h-2.5 rounded-full bg-[#27c93f]"></div>
        </div>
        <span class="text-gray-400 text-[10px] font-bold uppercase tracking-[0.2em]">Ghost_Cortex // Substrate_Output</span>
    </div>
    
    <div bind:this={terminalContainer} class="flex-1 p-4 overflow-y-auto whitespace-pre-wrap break-words leading-relaxed scrollbar-thin">
        {#each terminal.logs as log}
            <div class="{log.cls} mb-1.5 flex gap-3">
                <span class="opacity-20 shrink-0">[{log.timestamp}]</span>
                <span class="flex-1">{log.message}</span>
            </div>
        {:else}
            <div class="text-gray-700 italic">Awaiting neural uplink...</div>
        {/each}
        
        {#if terminal.isGhostTyping}
            <div class="text-cyan-400 animate-pulse mt-2 flex gap-2">
                <span class="shrink-0">›</span>
                <span class="inline-block w-2 h-4 bg-cyan-400"></span>
            </div>
        {/if}
    </div>
</div>

<style>
    .scrollbar-thin::-webkit-scrollbar { width: 4px; }
    .scrollbar-thin::-webkit-scrollbar-thumb { background: #30363d; border-radius: 10px; }
</style>
