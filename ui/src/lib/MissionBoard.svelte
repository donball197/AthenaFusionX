<script>
  import { onMount } from 'svelte';
  let missions = [];
  let loading = true;

  async function refreshMissions() {
    try {
      const res = await fetch('/api/missions');
      if (res.ok) missions = await res.json();
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  }

  onMount(refreshMissions);
</script>

<div class="mission-container h-full">
  <div class="header">
    <span class="pulse"></span>
    <h3>DIRECTIVES</h3>
  </div>

  {#if loading}
    <p class="meta">Scanning for active directives...</p>
  {:else if missions.length === 0}
    <p class="meta">No active directives.</p>
  {:else}
    <div class="mission-list">
      {#each missions as mission}
        <div class="mission-card">
          <div class="id">{mission.id.replace('.txt', '').toUpperCase()}</div>
          <div class="desc">{mission.description}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .mission-container {
    font-family: 'JetBrains Mono', monospace;
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 1rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid #222;
  }
  .pulse {
    width: 9px;
    height: 9px;
    background: #00ffcc;
    border-radius: 50%;
    animation: pulse 2s infinite;
  }
  h3 {
    margin: 0;
    color: #00ffcc;
    font-size: 0.95rem;
    letter-spacing: 3px;
    text-transform: uppercase;
  }
  .meta {
    font-size: 0.75rem;
    color: #555;
  }
  .mission-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .mission-card {
    padding: 10px 12px;
    border-left: 3px solid #00ffcc;
    background: rgba(0, 255, 204, 0.04);
  }
  .id {
    font-size: 0.7rem;
    color: #00ffcc;
    opacity: 0.85;
  }
  .desc {
    font-size: 0.85rem;
    color: #ddd;
    margin-top: 4px;
  }
  @keyframes pulse {
    0% { box-shadow: 0 0 0 0 rgba(0, 255, 204, 0.8); }
    70% { box-shadow: 0 0 0 14px rgba(0, 255, 204, 0); }
    100% { box-shadow: 0 0 0 0 rgba(0, 255, 204, 0); }
  }
</style>
