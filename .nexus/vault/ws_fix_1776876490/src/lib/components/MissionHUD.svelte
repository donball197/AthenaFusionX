<script>
    import { fly } from 'svelte/transition';
    let { activeMission = "IDLE", steps = [] } = $props();
</script>

<div class="mission-hud glass-panel p-4 font-mono">
    <div class="flex justify-between items-center border-b border-cyan-500/20 pb-2 mb-4">
        <span class="text-[10px] text-cyan-400 font-bold tracking-widest">CURRENT_MISSION: {activeMission}</span>
        <div class="recording-dot"></div>
    </div>

    <div class="step-list space-y-2">
        {#each steps as step, i}
            <div class="step-item flex gap-3 items-start" in:fly={{ x: -10 }}>
                <span class="text-[8px] text-cyan-800">[{i+1}]</span>
                <div class="flex-1">
                    <p class="text-[10px] text-gray-300">{step.text}</p>
                    <div class="h-[1px] bg-gradient-to-r from-cyan-500/50 to-transparent mt-1"></div>
                </div>
            </div>
        {/each}
    </div>
</div>

<style>
    .recording-dot { width: 6px; height: 6px; background: #f87171; border-radius: 50%; animation: pulse 1s infinite; }
    @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
</style>
