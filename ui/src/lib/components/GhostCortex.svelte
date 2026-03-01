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
    let ghostLogs = [];           // capped at 15 entries
    let socket;

    // Cheap thinking indicator: just cycling dots, no heavy interval
    let thinkingDots = "";
    let thinkingInterval;

    const MAX_GHOST_LOGS = 15;

    function generateThinkingDots() {
        thinkingDots = thinkingDots.length < 3 ? thinkingDots + "." : "";
    }

    function connectGhost() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${protocol}//${window.location.host}/ws/ghost`);

        socket.onmessage = (event) => {
            ghostLogs = [...ghostLogs, event.data.trim()].slice(-MAX_GHOST_LOGS);
        };

        socket.onclose = () => {
            if (active) setTimeout(connectGhost, 8000); // slower reconnect
        };
    }

    $: if (isThinking) {
        if (!thinkingInterval) {
            thinkingInterval = setInterval(generateThinkingDots, 600);
        }
    } else {
        if (thinkingInterval) {
            clearInterval(thinkingInterval);
            thinkingInterval = null;
            thinkingDots = "";
        }
    }

    function handleKey(e) {
        if (e.key === 'Enter' && input.trim() && !isThinking) {
            onCommand(input.trim());
            input = "";
            mode = "NEURAL";
        }
    }

    onMount(() => {
        if (active) connectGhost();
    });

    onDestroy(() => {
        if (thinkingInterval) clearInterval(thinkingInterval);
        if (socket) socket.close();
    });
</script>

{#if active}
<div 
    class="ghost-root shadow-xl bg-black/92 border border-[#30363d] border-l-2 border-l-purple-600 backdrop-blur-sm"
    in:fly={{ y: 24, duration: 280 }}
>
    <!-- Header – smaller, less padding -->
    <div class="flex items-center justify-between px-3 py-1.5 bg-[#161b22] border-b border-[#30363d] text-[9px]">
        <div class="flex gap-6">
            <button 
                class="{mode === 'NEURAL' ? 'text-purple-400 border-b border-purple-500' : 'text-gray-500'} font-bold uppercase tracking-tight"
                on:click={() => mode = "NEURAL"}
            >
                NEURAL
            </button>
            <button 
                class="{mode === 'SYSTEM' ? 'text-purple-400 border-b border-purple-500' : 'text-gray-500'} font-bold uppercase tracking-tight"
                on:click={() => mode = "SYSTEM"}
            >
                SYSTEM
            </button>
        </div>

        <div class="flex items-center gap-3">
            <div 
                class="safety-valve text-[7px] px-1.5 py-0.5 rounded border font-bold uppercase"
                class:testing={isThinking}
                class:fail={logs.toLowerCase().includes("error")}
            >
                SHIELD
            </div>
            <button 
                class="text-[8px] px-2 py-0.5 bg-black border border-purple-900/40 text-purple-400 hover:bg-purple-950/40 rounded"
                on:click|stopPropagation={onToggleFiles}
            >
                [ FILES ]
            </button>
            <div 
                class="w-2 h-2 rounded-full"
                class:bg-green-500={!isThinking}
                class:bg-purple-500={isThinking}
                class:animate-pulse={isThinking}
            ></div>
        </div>
    </div>

    <!-- Display – smaller font, tighter padding -->
    <div class="h-20 p-2.5 overflow-y-auto text-[9.5px] custom-scrollbar">
        {#if mode === "SYSTEM"}
            <div class="text-green-700/90 font-mono whitespace-pre leading-tight" in:fade={{ duration: 180 }}>
                <div class="text-[8px] text-blue-400 mb-1 font-bold">DRONE_TELEMETRY // {version}</div>
                {#each ghostLogs as log}
                    <div class="border-l border-green-900/30 pl-1.5 mb-0.5 opacity-90">{log}</div>
                {/each}
                {#if ghostLogs.length === 0}
                    <div class="italic text-gray-600 text-center py-2">Awaiting drone signal...</div>
                {/if}
            </div>
        {:else}
            <div class="text-gray-200 leading-tight" in:fade={{ duration: 180 }}>
                {#if isThinking}
                    <div class="text-purple-400 font-mono">
                        > ANALYZING{thinkingDots}
                    </div>
                {:else if response}
                    <div class="whitespace-pre-wrap">{response}</div>
                {:else}
                    <div class="text-gray-600 italic">UPLINK STABLE...</div>
                {/if}
            </div>
        {/if}
    </div>

    <!-- Input – minimal padding -->
    <div class="flex items-center px-3 py-1.5 bg-[#0a0a0a] border-t border-[#30363d]">
        <span class="text-purple-500 font-bold mr-2 text-[10px]">>></span>
        <input 
            bind:value={input}
            on:keydown={handleKey}
            placeholder={isThinking ? "Nexus processing..." : "Interrogate..."}
            disabled={isThinking}
            class="flex-1 bg-transparent outline-none text-white placeholder-gray-700 text-[10.5px]"
            spellcheck="false"
            autocomplete="off"
        />
    </div>
</div>
{/if}

<style>
    .ghost-root {
        position: absolute;
        bottom: 36px;
        left: 16px;
        width: 400px;
        font-family: 'Courier New', monospace;
        z-index: 100;
    }

    .safety-valve {
        background: #21262d;
        border: 1px solid #30363d;
        color: #8b949e;
    }
    .safety-valve.testing {
        color: #a855f7;
        border-color: #a855f7;
        animation: pulse 2s infinite;
    }
    .safety-valve.fail {
        color: #f85149;
        border-color: #f85149;
        background: rgba(248,81,73,0.08);
    }

    .custom-scrollbar::-webkit-scrollbar {
        width: 2px;
    }
    .custom-scrollbar::-webkit-scrollbar-thumb {
        background: #a855f7;
        border-radius: 1px;
    }

    @keyframes pulse {
        0%, 100% { opacity: 1; }
        50%      { opacity: 0.4; }
    }
</style>
