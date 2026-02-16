<script>
  export let swarm = [];
</script>

<div class="flex flex-col gap-3">
  {#each swarm as agent}
    <div class="flex items-center gap-3 p-2 border border-[#30363d] bg-black rounded relative overflow-hidden group">
      {#if agent.target !== "IDLE"}
        <div class="absolute left-3 top-1/2 -translate-y-1/2 w-2 h-2 bg-neon-green rounded-full animate-ping opacity-75"></div>
      {/if}
      
      <div class="w-2 h-2 rounded-full z-10 {agent.target === 'IDLE' ? 'bg-gray-700' : 'bg-neon-green shadow-[0_0_8px_#39FF14]'}"></div>
      
      <div class="flex-1">
        <div class="text-[9px] text-gray-400 font-bold uppercase tracking-tighter flex justify-between">
          <span>Agent_{agent.id.toString().padStart(2, '0')}</span>
          <span class={agent.target === 'IDLE' ? 'text-gray-700' : 'text-neon-green'}>{agent.target}</span>
        </div>
        <div class="h-1 w-full bg-gray-900 rounded-full mt-1 overflow-hidden">
          <div class="h-full bg-neon-green transition-all duration-1000" style="width: {agent.target === 'IDLE' ? '0%' : '100%'}"></div>
        </div>
      </div>
    </div>
  {/each}
</div>

<style>
  @keyframes ping {
    75%, 100% {
      transform: scale(3.5);
      opacity: 0;
    }
  }
  .animate-ping {
    animation: ping 1.5s cubic-bezier(0, 0, 0.2, 1) infinite;
  }
  .bg-neon-green {
    background-color: #39FF14;
  }
</style>
