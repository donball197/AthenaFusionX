<script>
    import { fly, fade } from 'svelte/transition';
    import { onDestroy } from 'svelte';
    import SoundFx from '$lib/SoundFx.js';

    export let logs = "";
    export let response = "";
    export let isThinking = false;
    export let active = false;
    export let onCommand = (cmd) => {};
    export let onToggleFiles; 
    export let version = "v0.41.2";

    let mode = "NEURAL"; 
    let input = "";

    let processingLogs = [];
    let interval;
    const hexChars = "0123456789ABCDEF";
    const systemTasks = ["ANALYZING AST", "VECTOR SEARCH", "OPTIMIZING GATES", "DECRYPTING", "SHARD SYNC"];

    function generateLog() {
        const hex = "0x" + Array(4).fill(0).map(() => hexChars[Math.floor(Math.random() * 16)]).join("");
        const task = systemTasks[Math.floor(Math.random() * systemTasks.length)];
        return `${hex} // ${task}...`;
    }

    $: if (isThinking) {
        if (!interval) {
            processingLogs = [generateLog()];
            interval = setInterval(() => {
                processingLogs = [...processingLogs.slice(-3), generateLog()];
            }, 150);
        }
    } else {
        if (interval) { clearInterval(interval); interval = null; }
    }

    function handleKey(e) {
        if (e.key === 'Enter' && input.trim()) {
            onCommand(input);
            input = "";
            mode = "NEURAL";
        }
    }

    onDestroy(() => { if (interval) clearInterval(interval); });
</script>

{#if active}
<div class="ghost-cortex-root shadow-2xl" in:fly={{ y: 20, duration: 400 }}>
    <div class="header">
        <div class="tabs">
            <button class={mode === "NEURAL" ? "active" : ""} on:click={() => mode = "NEURAL"}>NEURAL_LINK</button>
            <button class={mode === "SYSTEM" ? "active" : ""} on:click={() => mode = "SYSTEM"}>SYSTEM_LOGS</button>
        </div>
        <div class="actions">
             <button class="file-btn" on:click|stopPropagation={onToggleFiles}>[ FILES ]</button>
             <div class="led {isThinking ? 'thinking' : 'idle'}"></div>
        </div>
    </div>

    <div class="display-area custom-scrollbar">
        {#if mode === "SYSTEM"}
            <div class="logs" in:fade>
                <div class="version-tag">STDOUT // {version}</div>
                {logs}
            </div>
        {:else}
            <div class="neural-feed" in:fade>
                {#if isThinking}
                    <div class="thinking-stream">
                        {#each processingLogs as log}
                            <div class="log-line">> {log}</div>
                        {/each}
                    </div>
                {:else if response}
                    <div class="response-body">{response}</div>
                {:else}
                    <div class="dim">AWAITING NEURAL UPLINK...</div>
                {/if}
            </div>
        {/if}
    </div>

    <div class="input-line">
        <span class="prompt">GHOST_CMD></span>
        <input bind:value={input} on:keydown={handleKey} placeholder="Ask Gemma..." />
    </div>
</div>
{/if}

<style>
    .ghost-cortex-root {
        position: absolute; 
        bottom: 40px; 
        left: 20px; 
        width: 420px;
        background-color: rgba(10, 10, 10, 0.9); 
        border: 1px solid #30363d;
        border-left: 2px solid #a855f7; 
        backdrop-filter: blur(10px);
        display: flex; 
        flex-direction: column; 
        z-index: 100;
    }
    .header { 
        display: flex; 
        justify-content: space-between; 
        padding: 6px 10px;
        background-color: #161b22; 
        border-bottom: 1px solid #30363d;
    }
    .tabs { display: flex; gap: 12px; }
    .tabs button { 
        background: transparent; 
        border: none; 
        color: #484f58; 
        font-size: 9px;
        font-weight: bold; 
        cursor: pointer; 
        padding: 2px 0;
    }
    .tabs button.active { color: #a855f7; border-bottom: 1px solid #a855f7; }
    .file-btn {
        background-color: rgba(168, 85, 247, 0.1); 
        border: 1px solid rgba(168, 85, 247, 0.3);
        color: #a855f7; 
        font-size: 8px; 
        padding: 1px 6px; 
        cursor: pointer; 
        margin-right: 10px;
    }
    /* FIX: NO TAILWIND IN STYLE BLOCK */
    .led { 
        width: 6px; 
        height: 6px; 
        border-radius: 99px; 
        display: inline-block; 
    }
    .led.idle { background-color: #238636; }
    .led.thinking { background-color: #a855f7; animation: pulse 1s infinite; }
    
    .display-area { height: 100px; padding: 10px; overflow-y: auto; font-family: monospace; font-size: 10px; }
    .logs { color: #39FF14; opacity: 0.6; white-space: pre-wrap; font-style: italic; }
    .response-body { color: #e6edf3; line-height: 1.4; }
    .log-line { color: #a855f7; opacity: 0.8; }
    .dim { color: #30363d; }
    .version-tag { font-size: 8px; color: #30363d; margin-bottom: 4px; }

    .input-line { 
        display: flex; 
        align-items: center; 
        padding: 8px 10px; 
        background-color: #000; 
        border-top: 1px solid #30363d; 
    }
    .prompt { color: #a855f7; font-weight: bold; margin-right: 8px; font-size: 10px; }
    input { background: transparent; border: none; color: #fff; font-size: 11px; width: 100%; outline: none; }

    @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background-color: #a855f7; }
</style>
