<script>
    import { afterUpdate } from 'svelte';
    import { terminalLogs, systemVitals } from '$lib/store.js';
    import { runTask, spawnDrone } from '$lib/api.js';
    import { pushTermLine } from './NexusWS.js';

    let logContainer;
    let commandInput = '';
    let isProcessing = false;

    afterUpdate(() => {
        if (logContainer) {
            logContainer.scrollTop = logContainer.scrollHeight;
        }
    });

    async function handleCommand(e) {
        if (e.key === 'Enter' && commandInput.trim()) {
            const cmd = commandInput.trim();
            commandInput = '';
            
            // Mirror input to terminal
            pushTermLine(`\x1b[36mnexus_visor>\x1b[0m ${cmd}`);
            
            // ROUTE: Drone (Background) vs Task (Foreground)
            if (cmd.toLowerCase().startsWith('drone ')) {
                const directive = cmd.substring(6);
                const ok = await spawnDrone(directive);
                if (!ok) pushTermLine('!! [SYSTEM_FAULT]: Failed to dispatch drone.');
                return;
            }
            
            // Default: Foreground Task
            isProcessing = true;
            const result = await runTask(cmd);
            pushTermLine(result);
            isProcessing = false;
        }
    }
</script>

<div class="h-full flex flex-col bg-black font-mono text-sm border-t border-[#00FFFF]/30">
    <div class="flex items-center justify-between px-3 py-1 bg-[#0A1A1F] border-b border-[#00FFFF]/20">
        <span class="text-[#00FFFF] font-bold tracking-wider text-xs">NEXUS TERMINAL v2.0</span>
        <div class="flex gap-4 text-[10px] text-cyan-400/70">
            <span>CPU: {$systemVitals.cpu_usage.toFixed(1)}%</span>
            <span>RAM: {$systemVitals.ram_usage}</span>
            <span class={$systemVitals.temperature > 68 ? 'text-red-400 animate-pulse' : ''}>
                TEMP: {$systemVitals.temperature.toFixed(1)}°C
            </span>
        </div>
    </div>

    <div bind:this={logContainer} class="flex-1 overflow-y-auto p-2 space-y-1 scrollbar-thin scrollbar-thumb-cyan-900 scrollbar-track-black">
        {#each $terminalLogs as log}
            <div class="whitespace-pre-wrap leading-tight">{@html log.html}</div>
        {/each}
        {#if isProcessing}
            <div class="text-yellow-400 animate-pulse">>> [NEURAL UPLINK ACTIVE] Processing...</div>
        {/if}
    </div>

    <div class="flex items-center px-2 py-1 bg-[#050A0F]">
        <span class="text-cyan-400 mr-2">nexus_visor></span>
        <input 
            type="text" 
            bind:value={commandInput} 
            on:keydown={handleCommand}
            disabled={isProcessing}
            class="flex-1 bg-transparent text-gray-300 focus:outline-none disabled:opacity-50"
            placeholder={isProcessing ? "Awaiting neural response..." : "Enter system directive..."}
        />
    </div>
</div>
