<script>
    // GhostCortex.svelte — v1.1 [IMPORT_FIX + PULSE_DOT_CSS]
    // FIX: Was importing terminalState alias from store.svelte — now imports
    //      terminal directly from terminal.svelte.ts for consistency.
    // FIX: .pulse-dot had no CSS definition — added.
    import { terminal } from '$lib/stores/terminal.svelte';
    import { layout } from '$lib/core/store.svelte';
    import { fly, fade } from 'svelte/transition';

    let buffer   = $derived(terminal.ghostBuffer);
    let isTyping = $derived(terminal.isGhostTyping);
</script>

{#if layout.ghostVisible && (isTyping || buffer.length > 35)}
    <div class="cortex-overlay glass-panel" in:fly={{ y: 20 }} out:fade>
        <div class="header">
            <span class="pulse-dot"></span>
            GHOST_CORTEX_MONOLOGUE
        </div>
        <div class="content">
            <span class="ghost-text">{buffer}</span>
            {#if isTyping}<span class="cursor">_</span>{/if}
        </div>
    </div>
{/if}

<style>
    .cortex-overlay { position: absolute; bottom: 85px; right: 20px; width: 320px; background: rgba(2,2,10,0.9); border: 1px solid rgba(167,139,250,0.2); z-index: 1000; }
    .header {
        background: rgba(167,139,250,0.05); font-size: 7px; padding: 6px 10px;
        color: #a78bfa; font-weight: 900; display: flex; align-items: center; gap: 8px;
    }
    /* FIX: pulse-dot had no CSS */
    .pulse-dot {
        width: 6px; height: 6px; border-radius: 50%;
        background: #f87171;
        animation: dot-pulse 1s ease-in-out infinite;
        flex: none;
    }
    @keyframes dot-pulse { 0%,100%{opacity:1;} 50%{opacity:.25;} }
    .content { padding: 12px; font-size: 10px; color: #e2e8f0; font-family: 'JetBrains Mono', monospace; max-height: 200px; overflow-y: auto; }
    .ghost-text { white-space: pre-wrap; word-break: break-word; color: #c4b5fd; }
    .cursor { color: #a78bfa; animation: blink 1s infinite; }
    @keyframes blink { 50%{opacity:0;} }
</style>
