<script>
    import { fly, fade } from 'svelte/transition';
    import { onDestroy, onMount } from 'svelte';

    export let logs = "";
    export let response = "";
    export let isThinking = false;
    export let active = false;
    export let onCommand = (cmd) => {};
    export let onToggleFiles; 
    export let version = "v0.41.5";

    let mode = "NEURAL"; 
    let input = "";
    let ghostLogs = [];
    let socket;

    let processingLogs = [];
    let interval;
    const hexChars = "0123456789ABCDEF";
    const systemTasks = ["ANALYZING AST", "VECTOR SEARCH", "OPTIMIZING GATES", "DECRYPTING"];

    function generateLog() {
        const hex = "0x" + Array(4).fill(0).map(() => hexChars[Math.floor(Math.random() * 16)]).join("");
        const task = systemTasks[Math.floor(Math.random() * systemTasks.length)];
        return `${hex} // ${task}...`;
    }

    function connectGhost() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${protocol}//${window.location.host}/ws/ghost`);
        socket.onmessage = (event) => {
            ghostLogs = [...ghostLogs.slice(-20), event.data]; // Keep last 20 drone logs
        };
        socket.onclose = () => { if (active) setTimeout(connectGhost, 5000); };
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

    onMount(() => { connectGhost(); });
    onDestroy(() => { 
        if (interval) clearInterval(interval); 
        if (socket) socket.close();
    });
</script>

{#if active}
<div class="ghost-root shadow-2xl" in:fly={{ y: 20, duration: 400 }}>
    <div class="header">
        <div class="tabs">
            <button class={mode === "NEURAL" ? "active" : ""} on:click={() => mode = "NEURAL"}>NEURAL</button>
            <button class={mode === "SYSTEM" ? "active" : ""} on:click={() => mode = "SYSTEM"}>SYSTEM</button>
        </div>
        <div class="actions">
             <div class="safety-valve" class:testing={isThinking} class:fail={logs.includes("ERROR")}>SHIELD</div>
             <button class="file-btn" on:click|stopPropagation={onToggleFiles}>[ FILES ]</button>
             <div class="led" class:thinking={isThinking} class:idle={!isThinking}></div>
        </div>
    </div>

    <div class="display custom-scrollbar">
        {#if mode === "SYSTEM"}
            <div class="logs" in:fade>
                <div class="version-tag">DRONE_TELEMETRY // {version}</div>
                {#each ghostLogs as log}
                    <div class="ghost-line">{log}</div>
                {/each}
                {#if ghostLogs.length === 0}
                    <div class="dim italic">Waiting for drone heartbeat...</div>
                {/if}
            </div>
        {:else}
            <div class="neural-feed" in:fade>
                {#if isThinking}
                    <div class="thinking">
                        {#each processingLogs as log}
                            <div>> {log}</div>
                        {/each}
                    </div>
                {:else if response}
                    <div class="response">{response}</div>
                {:else}
                    <div class="dim">UPLINK STABLE...</div>
                {/if}
            </div>
        {/if}
    </div>

    <div class="input-line">
        <span class="prompt">>></span>
        <input bind:value={input} on:keydown={handleKey} placeholder="Interrogate..." />
    </div>
</div>
{/if}

<style>
    .ghost-root {
        position: absolute; bottom: 40px; left: 20px; width: 420px;
        background-color: rgba(10, 10, 10, 0.9); border: 1px solid #30363d;
        border-left: 2px solid #a855f7; backdrop-filter: blur(10px);
        display: flex; flex-direction: column; z-index: 100; font-family: monospace;
    }
    .header { display: flex; justify-content: space-between; padding: 6px 10px; background-color: #161b22; }
    .tabs { display: flex; gap: 10px; }
    .tabs button { background: transparent; border: none; color: #484f58; font-size: 9px; font-weight: bold; cursor: pointer; }
    .tabs button.active { color: #a855f7; border-bottom: 1px solid #a855f7; }
    .actions { display: flex; align-items: center; gap: 8px; }
    .safety-valve { font-size: 8px; font-weight: bold; padding: 1px 4px; border-radius: 2px; background-color: #21262d; color: #8b949e; border: 1px solid #30363d; }
    .safety-valve.testing { color: #a855f7; border-color: #a855f7; animation: pulse 1s infinite; }
    .safety-valve.fail { color: #f85149; border-color: #f85149; background-color: rgba(248, 81, 73, 0.1); }
    .file-btn { background-color: #000; border: 1px solid #30363d; color: #a855f7; font-size: 8px; cursor: pointer; }
    .led { width: 6px; height: 6px; border-radius: 50%; display: inline-block; }
    .led.idle { background-color: #238636; }
    .led.thinking { background-color: #a855f7; animation: pulse 1s infinite; }
    .display { height: 100px; padding: 10px; overflow-y: auto; font-size: 10px; }
    .logs { color: #39FF14; opacity: 0.8; white-space: pre-wrap; }
    .ghost-line { margin-bottom: 2px; border-left: 1px solid rgba(57, 255, 20, 0.2); padding-left: 5px; }
    .response { color: #e6edf3; line-height: 1.4; }
    .thinking { color: #a855f7; opacity: 0.8; }
    .dim { color: #30363d; }
    .version-tag { font-size: 8px; color: #58a6ff; margin-bottom: 6px; font-weight: bold; }
    .input-line { display: flex; align-items: center; padding: 8px 10px; background-color: #000; border-top: 1px solid #30363d; }
    .prompt { color: #a855f7; font-weight: bold; margin-right: 8px; }
    input { background: transparent; border: none; color: #fff; font-size: 11px; width: 100%; outline: none; }
    @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background-color: #a855f7; }
</style>
