<script>
    import AgentMap from '$lib/components/AgentMap.svelte';
    let swarm = $state(Array.from({length: 12}, (_, i) => ({ id: i, target: i < 3 ? "ACTIVE" : "IDLE" })));
</script>

<div class="p-8 h-full bg-black overflow-y-auto">
    <header class="flex justify-between items-baseline mb-8">
        <h1 class="text-purple-400 font-bold tracking-widest uppercase italic">Swarm_Reflex_Lab</h1>
        <span class="text-[10px] text-purple-900">NODE_COUNT: 68</span>
    </header>
    <AgentMap {swarm} />
</div>
