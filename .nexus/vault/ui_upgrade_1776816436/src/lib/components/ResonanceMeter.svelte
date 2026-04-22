<script lang="ts">
  import { onMount } from 'svelte';

  let resonance = 0.5;
  let activeVmos = 0;
  let temp = 42.0;

  onMount(() => {
    const ws = new WebSocket('ws://localhost:3000/ws/telemetry');
    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      if (data.type_id === 'HEARTBEAT') {
        resonance = data.resonance;
        activeVmos = data.active_vmos;
        temp = data.temperature;
      }
    };

    setInterval(async () => {
      const res = await fetch('/api/ghost');
      const ghost = await res.json();
      resonance = ghost.resonance;
    }, 2500);
  });
</script>

<div class="resonance-meter">
  <svg width="320" height="320" viewBox="0 0 320 320">
    <circle cx="160" cy="160" r="140" fill="none" stroke="#222" stroke-width="25"/>
    <circle cx="160" cy="160" r="140" fill="none"
            stroke="hsl({(resonance * 120) % 360}, 100%, 65%)"
            stroke-width="25"
            stroke-dasharray="879.6"
            stroke-dashoffset="{879.6 * (1 - resonance)}"
            transform="rotate(-90 160 160)"/>
    <text x="160" y="155" text-anchor="middle" font-size="42" fill="#0ff" font-family="monospace">
      {Math.round(resonance * 100)}%
    </text>
    <text x="160" y="190" text-anchor="middle" font-size="18" fill="#0ff" opacity="0.7">
      RESONANCE
    </text>
  </svg>

  <div class="stats">
    <div>VMOs: <span class="value">{activeVmos}</span></div>
    <div>Temp: <span class="value">{temp.toFixed(1)}°C</span></div>
  </div>
</div>

<style>
  .resonance-meter { text-align: center; margin: 2rem auto; }
  .stats { display: flex; justify-content: center; gap: 2rem; margin-top: 1rem; font-family: monospace; }
  .value { color: #0ff; font-weight: bold; }
</style>
