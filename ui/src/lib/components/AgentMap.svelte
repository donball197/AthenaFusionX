<script>
    let { swarm = [] } = $props();
</script>

<div class="flex flex-col gap-2">
  {#each swarm as agent}
    <div class="group flex items-center gap-4 p-3 bg-black border border-cyan-900/20 rounded hover:border-cyan-400 transition-all duration-300">
      <div class="relative flex-shrink-0">
        {#if agent.target !== "IDLE"}
          <div class="absolute inset-0 bg-cyan-400 rounded-full animate-ping opacity-20"></div>
        {/if}
        <div class="w-2 h-2 rounded-full {agent.target === 'IDLE' ? 'bg-gray-800' : 'bg-cyan-400 shadow-[0_0_10px_#00FFFF]'}"></div>
      </div>

      <div class="flex-1 min-w-0">
        <div class="flex justify-between items-baseline text-[9px] uppercase tracking-widest font-black">
          <span class="text-gray-600">AGENT_{agent.id.toString().padStart(2, '0')}</span>
          <span class={agent.target === 'IDLE' ? 'text-gray-700' : 'text-cyan-400'}>{agent.target}</span>
        </div>
        <div class="mt-2 h-0.5 bg-[#111] rounded-full overflow-hidden">
          <div 
            class="h-full bg-cyan-500 transition-all duration-1000"
            style="width: {agent.target === 'IDLE' ? '5%' : '100%'}">
          </div>
        </div>
      </div>
    </div>
  {/each}
</div>
