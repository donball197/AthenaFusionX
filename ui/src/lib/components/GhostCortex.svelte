<script>
    import { ghostThinking } from '$lib/store';
    import { afterUpdate } from 'svelte';
    import { WS } from '$lib/components/NexusWS';

    // Props passed from Dashboard
    export let logs = [];
    export let response = "";
    export let active = false;

    let container;

    // Auto-scroll to bottom as AI types
    afterUpdate(() => {
        if (container) {
            container.scrollTop = container.scrollHeight;
        }
    });

    function acceptPatch() {
        WS.send({ type: "action", action: "accept_snapshot", data: logs.join("") });
    }

    function rejectPatch() {
        WS.send({ type: "action", action: "reject_patch" });
    }

    function saveAndBuild() {
        WS.send({ type: "action", action: "cargo_build" });
    }
</script>

<div class="ghost-panel">
    <div class="header">
        <span class="title">GHOST CORTEX</span>
        <span class="pulse" class:active={active || $ghostThinking}></span>
    </div>
    
    <div class="content" bind:this={container}>
        {#if logs.length > 0}
            <pre>{logs.join("")}</pre>
        {:else if response}
            <pre>{response}</pre>
        {:else}
            <pre class="text-gray-600">>> AWAITING NEURAL STREAM...</pre>
        {/if}
    </div>

    {#if !$ghostThinking && (logs.length > 0 || response.length > 10)}
    <div class="action-tabs">
        <button class="btn accept" on:click={acceptPatch}>[ ACCEPT ]</button>
        <button class="btn build" on:click={saveAndBuild}>[ BUILD ]</button>
        <button class="btn reject" on:click={rejectPatch}>[ REJECT ]</button>
    </div>
    {/if}
</div>

<style>
    .ghost-panel { display: flex; flex-direction: column; height: 100%; min-height: 0; overflow: hidden; background: #0d1117; }
    .header { display: flex; justify-content: space-between; align-items: center; padding: 6px 12px; background: #161b22; border-bottom: 1px solid #30363d; }
    .title { font-size: 10px; font-weight: 600; color: #8b949e; letter-spacing: 1px; }
    .pulse { width: 8px; height: 8px; border-radius: 50%; background: #30363d; }
    .pulse.active { background: #58a6ff; box-shadow: 0 0 8px #58a6ff; animation: throb 1.5s infinite; }
    .content { flex: 1; padding: 8px; overflow-y: auto; min-height: 0; color: #c9d1d9; font-size: 11px; line-height: 1.4; }
    pre { white-space: pre-wrap; font-family: monospace; margin: 0; }
    .action-tabs { display: flex; gap: 4px; padding: 6px; background: #161b22; border-top: 1px solid #30363d; }
    .btn { flex: 1; background: #0d1117; border: 1px solid #30363d; color: #8b949e; cursor: pointer; padding: 8px; font-size: 9px; font-family: monospace; }
    .btn:hover { background: #21262d; color: #c9d1d9; }
    .btn.accept:hover { border-color: #238636; color: #3fb950; }
    @keyframes throb { 0% { opacity: 0.5; } 50% { opacity: 1; } 100% { opacity: 0.5; } }
</style>
