<script>
    export let swarm = [];
    function getCoords(target) {
        if (!target || target === "IDLE") return { x: 50, y: 50, active: false };
        let hash = 0;
        for (let i = 0; i < target.length; i++) hash = target.charCodeAt(i) + ((hash << 5) - hash);
        return { x: Math.abs((hash % 80) + 10), y: Math.abs(((hash >> 8) % 80) + 10), active: true };
    }
</script>
<div class="relative h-[200px] bg-black border border-[#1a1a1a] overflow-hidden mb-4">
    <div class="absolute inset-0 bg-[radial-gradient(#111_1px,transparent_1px)] [background-size:20px_20px]"></div>
    <div class="absolute w-full h-[2px] bg-gradient-to-b from-transparent via-green-500 to-transparent opacity-10 animate-[scan_4s_linear_infinite]"></div>
    {#each swarm as agent}
        {@const pos = getCoords(agent.target)}
        <div class="absolute w-1 h-1 bg-[#222] rounded-full transition-all duration-700" 
             class:bg-neon-green={pos.active} 
             class:shadow-[0_0_8px_#0f0]={pos.active}
             style="left: {pos.x}%; top: {pos.y}%;"></div>
    {/each}
</div>
