<script>
  import { onMount } from 'svelte';
  let missions = $state([]);
  let loading = $state(true);

  async function refreshMissions() {
    try {
      const res = await fetch('/api/missions');
      if (res.ok) {
          missions = await res.json();
      }
    } catch (e) {
      console.error("[MISSION_FAULT]", e);
    } finally {
      loading = false;
    }
  }

  onMount(refreshMissions);
</script>

<div class="mission-container h-full p-4 font-mono">
  <div class="header flex items-center gap-3 border-b border-cyan-900/50 pb-2 mb-4">
    <div class="w-2 h-2 rounded-full bg-cyan-400 animate-pulse"></div>
    <h3 class="text-cyan-400 tracking-widest text-sm font-bold uppercase">Active Directives</h3>
  </div>

  {#if loading}
    <p class="text-xs text-cyan-800">Scanning neural queue...</p>
  {:else if missions.length === 0}
    <p class="text-xs text-gray-600 italic text-center py-10">No active directives found.</p>
  {:else}
    <div class="mission-list flex flex-col gap-3 overflow-y-auto">
      {#each missions as mission}
        <div class="mission-card p-3 border-l-2 border-cyan-500 bg-cyan-500/5 group hover:bg-cyan-500/10 transition-colors">
          <div class="flex justify-between items-start mb-1">
              <span class="text-[10px] text-cyan-600 font-bold uppercase">{mission.id.slice(0,8)}</span>
              <span class="text-[9px] px-1 bg-cyan-900/30 text-cyan-400 uppercase">{mission.status}</span>
          </div>
          <!-- FIX: backend uses .goal, not .description -->
          <div class="text-[12px] text-gray-200 group-hover:text-white">{mission.goal}</div>
          <div class="text-[9px] text-cyan-900 mt-2">{new Date(mission.created_at * 1000).toLocaleString()}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>
