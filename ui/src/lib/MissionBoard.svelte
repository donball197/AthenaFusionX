<script>
  import { onMount } from 'svelte';
  let missions = [];
  let loading = true;

  async function refreshMissions() {
    try {
      const res = await fetch('/api/missions');
      if (res.ok) {
        missions = await res.json();
      }
    } catch (e) {
      console.error("Mission Control Offline:", e);
    } finally {
      loading = false;
    }
  }

  onMount(refreshMissions);
</script>

<div class="mission-container">
  <div class="header">
    <span class="pulse"></span>
    <h3>DIRECTIVES</h3>
  </div>

  {#if loading}
    <p class="meta">Scanning missions...</p>
  {:else if missions.length === 0}
    <p class="meta">No active directives found.</p>
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
    padding: 1rem;
    background: rgba(10, 10, 15, 0.8);
    border: 1px solid #333;
    border-radius: 4px;
    font-family: 'JetBrains Mono', 'Courier New', monospace;
  }
  .header { display: flex; align-items: center; gap: 10px; margin-bottom: 1rem; }
  .pulse { width: 8px; height: 8px; background: #00ffcc; border-radius: 50%; animation: pulse 2s infinite; }
  h3 { margin: 0; color: #00ffcc; font-size: 0.9rem; letter-spacing: 2px; }
  .meta { font-size: 0.7rem; color: #666; }
  .mission-list { display: flex; flex-direction: column; gap: 8px; }
  .mission-card {
    padding: 8px;
    border-left: 2px solid #00ffcc;
    background: rgba(0, 255, 204, 0.05);
    transition: background 0.3s;
  }
  .mission-card:hover { background: rgba(0, 255, 204, 0.1); }
  .id { font-size: 0.65rem; color: #00ffcc; opacity: 0.8; margin-bottom: 4px; }
  .desc { font-size: 0.85rem; color: #e0e0e0; }

  @keyframes pulse {
    0% { box-shadow: 0 0 0 0 rgba(0, 255, 204, 0.7); }
    70% { box-shadow: 0 0 0 10px rgba(0, 255, 204, 0); }
    100% { box-shadow: 0 0 0 0 rgba(0, 255, 204, 0); }
  }
</style>
