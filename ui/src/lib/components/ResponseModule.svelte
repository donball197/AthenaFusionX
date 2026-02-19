<script>
    import { onMount, onDestroy } from 'svelte';
    import { fly, fade } from 'svelte/transition';
    import SoundFx from '$lib/SoundFx.js';

    export let response = "";
    export let isThinking = false;
    export let patchAvailable = false;
    export let onApplyPatch;

    // ANIMATION LOGIC
    let processingLogs = [];
    let interval;
    
    const hexChars = "0123456789ABCDEF";
    const systemTasks = [
        "CALIBRATING NEURAL WEIGHTS",
        "PARSING SYNTAX TREE",
        "OPTIMIZING LOGIC GATES",
        "SCANNING VECTOR DB",
        "VALIDATING MEMORY SHARDS",
        "DECRYPTING RESPONSE STREAM",
        "HANDSHAKE PROTOCOL: ACK",
        "BUFFERING OUTPUT STREAMS"
    ];

    function generateLog() {
        const timestamp = new Date().toISOString().split('T')[1].slice(0, 8);
        const hex = "0x" + Array(4).fill(0).map(() => hexChars[Math.floor(Math.random() * 16)]).join("");
        const task = systemTasks[Math.floor(Math.random() * systemTasks.length)];
        return `[${timestamp}] ${hex} // ${task}...`;
    }

    // React to thinking state
    $: if (isThinking) {
        if (!interval) {
            // FIX: Start with one log immediately so it doesn't look empty for 100ms
            processingLogs = [generateLog()];
            interval = setInterval(() => {
                processingLogs = [...processingLogs.slice(-7), generateLog()];
            }, 100); 
        }
    } else {
        if (interval) {
            clearInterval(interval);
            interval = null;
        }
    }

    onDestroy(() => { if (interval) clearInterval(interval); });
</script>

<div class="h-64 flex flex-col relative overflow-hidden bg-[#0d1117] border border-[#30363d] rounded">
    
    <div class="h-8 shrink-0 border-b border-[#30363d] bg-[#161b22] flex items-center justify-between px-3">
        <div class="flex items-center gap-2">
            <div class="w-2 h-2 rounded-full {isThinking ? 'bg-purple-500 animate-ping' : 'bg-green-500'}"></div>
            <span class="text-[9px] font-bold tracking-widest text-gray-400 uppercase">
                GEMMA_AGENT // {isThinking ? 'PROCESSING' : 'IDLE'}
            </span>
        </div>
        {#if patchAvailable}
            <span class="text-[8px] bg-green-900/40 text-green-400 border border-green-500/30 px-1.5 py-0.5 rounded animate-pulse">
                PATCH READY
            </span>
        {/if}
    </div>

    <div class="flex-1 overflow-y-auto custom-scrollbar p-4 font-mono text-[11px] leading-relaxed">
        
        {#if isThinking}
            <div class="flex flex-col gap-1 h-full justify-end pb-4" in:fade>
                {#each processingLogs as log, i}
                    <div class="font-mono text-[10px] tracking-tight whitespace-nowrap overflow-hidden transition-all duration-75"
                         style="opacity: {(i + 1) / processingLogs.length}; color: {i === processingLogs.length - 1 ? '#a855f7' : '#4b5563'}">
                        <span class="text-purple-500/50">></span> {log}
                    </div>
                {/each}
                <div class="mt-2 h-1 w-full bg-purple-900/20 rounded overflow-hidden">
                    <div class="h-full bg-purple-500/50 animate-progress"></div>
                </div>
            </div>
        
        {:else if response}
            <div class="text-gray-300 whitespace-pre-wrap markdown-body" in:fly={{ y: 20, duration: 300 }}>
                {response}
            </div>

            {#if patchAvailable}
                <div class="mt-6 border-t border-green-500/20 pt-4">
                    <button on:click={() => { SoundFx.success(); onApplyPatch(); }}
                            class="w-full group relative bg-green-950/20 hover:bg-green-900/30 border border-green-500/40 text-green-400 px-4 py-3 rounded text-xs font-bold tracking-widest transition-all overflow-hidden">
                        <div class="absolute inset-0 bg-green-500/10 translate-x-[-100%] group-hover:translate-x-[100%] transition-transform duration-700"></div>
                        <span class="relative flex items-center justify-center gap-2">
                            <span>[ EXECUTE PATCH ]</span>
                            <span class="text-[9px] opacity-50 font-normal">Apply Fix to Editor</span>
                        </span>
                    </button>
                </div>
            {/if}

        {:else}
            <div class="flex flex-col items-center justify-center h-full opacity-20 select-none">
                <svg class="w-12 h-12 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.384-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z"></path></svg>
                <span class="text-[9px] tracking-[0.2em]">AWAITING INPUT</span>
            </div>
        {/if}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 4px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #30363d; border-radius: 2px; }
    .custom-scrollbar::-webkit-scrollbar-track { background: transparent; }
    
    @keyframes progress {
        0% { width: 0%; }
        50% { width: 70%; }
        100% { width: 100%; }
    }
    .animate-progress {
        animation: progress 2s infinite ease-in-out;
    }
</style>
