<script>
  export let swarm = [];
</script>

<div class="flex flex-col gap-2.5">
  {#each swarm as agent}
    <div class="group flex items-center gap-4 p-3 bg-black border border-[#30363d] rounded hover:border-neon-green/60 transition-all duration-200">
      <div class="relative flex-shrink-0">
        {#if agent.target !== "IDLE"}
          <div class="absolute inset-0 bg-neon-green rounded-full animate-ping opacity-30"></div>
        {/if}
        <div class="w-3 h-3 rounded-full z-10 {agent.target === 'IDLE' ? 'bg-gray-700' : 'bg-neon-green shadow-[0_0_10px_#39FF14]'}"></div>
      </div>

      <div class="flex-1 min-w-0">
        <div class="flex justify-between items-baseline text-[10px] uppercase tracking-[1px] font-bold">
          <span class="text-gray-400">AGENT_{agent.id.toString().padStart(2, '0')}</span>
          <span class={agent.target === 'IDLE' ? 'text-gray-600' : 'text-neon-green'}>{agent.target}</span>
        </div>
        <div class="mt-2 h-1 bg-[#1a1a1a] rounded-full overflow-hidden">
          <div 
            class="h-full bg-neon-green transition-all duration-1000 ease-out"
            style="width: {agent.target === 'IDLE' ? '18%' : '100%'}">
          </div>
        </div>
      </div>
    </div>
  {/each}
</div>

<style>
  .bg-neon-green { background-color: #39FF14; }
</style>
