<script>
    let agents = Array.from({length: 68}, (_, i) => ({ id: i, status: i < 4 ? 'active' : 'idle' }));
</script>

<div class="p-4 grid grid-cols-12 gap-1 overflow-y-auto h-full bg-black">
    {#each agents as agent}
        <div class="aspect-square border border-white/5 flex items-center justify-center {agent.status === 'active' ? 'bg-cyan-500/20 border-cyan-500/50' : 'bg-gray-900/20'}">
            <span class="text-[8px] {agent.status === 'active' ? 'text-white' : 'text-gray-700'}">{agent.id}</span>
        </div>
    {/each}
</div>
