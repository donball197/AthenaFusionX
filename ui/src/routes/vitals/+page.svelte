<script>
    import { systemVitals } from '$lib/store';
    import Heartbeat from '$lib/components/Heartbeat.svelte';
</script>

<div class="p-8 h-full bg-black flex flex-col gap-8">
    <div class="grid grid-cols-4 gap-4">
        {#each Object.entries($systemVitals) as [key, val]}
            <div class="p-4 border border-cyan-900/20 bg-cyan-900/5">
                <span class="text-[9px] text-cyan-700 block mb-1 uppercase">{key}</span>
                <span class="text-xl font-black text-white">{typeof val === 'number' ? val.toFixed(2) : val}</span>
            </div>
        {/each}
    </div>
    <div class="flex-1 border border-cyan-900/30 p-10 flex items-center justify-center">
        <Heartbeat resonance={$systemVitals.resonance} />
    </div>
</div>
