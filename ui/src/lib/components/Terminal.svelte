<script>
    import { terminalLogs, systemVitals } from '$lib/store.js';
    import { runTask } from '$lib/api.js';
    import { pushTermLine } from './NexusWS.js';

    let logContainer = $state();
    let commandInput = $state('');
    let isProcessing = $state(false);

    // SUTURE: Replacing afterUpdate with $effect
    // This triggers every time terminalLogs is updated
    $effect(() => {
        if (logContainer && $terminalLogs) {
            logContainer.scrollTop = logContainer.scrollHeight;
        }
    });

    async function handleCommand(e) {
        if (e.key === 'Enter' && commandInput.trim()) {
            const cmd = commandInput.trim();
            commandInput = '';
            
            pushTermLine(`\x1b[36mnexus_visor>\x1b[0m ${cmd}`);
            
            isProcessing = true;
            try {
                const result = await runTask(cmd);
                pushTermLine(result.output || result);
            } catch (err) {
                pushTermLine(`!! [UPLINK_ERROR]: ${err.message}`, "text-red-500");
            } finally {
                isProcessing = false;
            }
        }
    }
</script>

<div class="h-full flex flex-col bg-black font-mono text-xs">
    <div class="flex-1 overflow-y-auto p-2 space-y-1 custom-scrollbar" bind:this={logContainer}>
        {#each $terminalLogs as log}
            <div class="whitespace-pre-wrap leading-tight">{@html log.html}</div>
        {/each}
        {#if isProcessing}
            <div class="text-yellow-400 animate-pulse">>> [NEURAL_UPLINK] Processing...</div>
        {/if}
    </div>

    <div class="flex items-center px-2 py-2 bg-[#050A0F] border-t border-cyan-900/20">
        <span class="text-cyan-400 mr-2 font-black">nexus_visor></span>
        <input 
            type="text" 
            bind:value={commandInput} 
            onkeydown={handleCommand}
            disabled={isProcessing}
            class="flex-1 bg-transparent text-gray-300 outline-none focus:ring-0"
            placeholder={isProcessing ? "Neural uplink active..." : "Enter system directive..."}
        />
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #0a2f3f; }
</style>
