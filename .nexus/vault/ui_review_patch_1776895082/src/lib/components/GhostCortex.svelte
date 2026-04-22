<script>
    import { terminalState, layout } from '$lib/core/store.svelte.js';
    import { fly, fade } from 'svelte/transition';
    
    let buffer = $derived(terminalState.buffers.GHOST_CORTEX);
    let isTyping = $derived(terminalState.isGhostTyping);
</script>

{#if layout.ghostCortexVisible && (isTyping || buffer.length > 35)}
<div class="cortex-overlay glass-panel" in:fly={{ y: 20 }} out:fade>
    <div class="header">
        <span class="pulse-dot"></span>
        GHOST_CORTEX_MONOLOGUE
    </div>
    <div class="content custom-scrollbar">
        <span class="ghost-text">{buffer}</span>
        {#if isTyping}<span class="cursor">_</span>{/if}
    </div>
</div>
{/if}

<style>
    .cortex-overlay {
        position: absolute; bottom: 85px; right: 20px; width: 320px;
        background: rgba(2, 2, 10, 0.9); border: 1px solid rgba(167, 139, 250, 0.2);
        z-index: 1000; box-shadow: 0 10px 40px rgba(0,0,0,0.8);
    }
    .header { 
        background: rgba(167, 139, 250, 0.05); font-size: 7px; padding: 6px 10px; 
        color: #a78bfa; font-weight: 900; letter-spacing: 2px;
        display: flex; align-items: center; gap: 8px; border-bottom: 1px solid rgba(167, 139, 250, 0.1);
    }
    .pulse-dot { width: 4px; height: 4px; background: #a78bfa; border-radius: 50%; animation: pulse 1s infinite; }
    @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
    .content { padding: 12px; font-size: 10px; color: #e2e8f0; font-family: 'JetBrains Mono', monospace; max-height: 200px; overflow-y: auto; line-height: 1.5; }
    .ghost-text { opacity: 0.8; }
    .cursor { color: #a78bfa; animation: blink 1s infinite; }
    @keyframes blink { 50% { opacity: 0; } }
</style>
