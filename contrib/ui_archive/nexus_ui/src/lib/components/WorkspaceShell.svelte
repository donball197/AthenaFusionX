<script lang="ts">
  import { telemetry } from '../core/telemetry.svelte.js';
</script>

<div class="cli-root">
  <header class="cli-header">
    <span class="status-indicator" class:online={telemetry.connected}>
      {telemetry.connected ? '● UPLINK_STABLE' : '○ UPLINK_SEVERED'}
    </span>
  </header>

  <div class="cli-terminal">
    {#each telemetry.logs as log}
      <pre class="log-line">{log}</pre>
    {:else}
      <div class="idle">AWAITING_SUBSTRATE_DATA...</div>
    {/each}
  </div>
</div>

<style>
  .cli-root { font-family: 'JetBrains Mono', monospace; background: #010105; height: 100vh; padding: 1rem; color: #00ffff; }
  .status-indicator { font-size: 10px; font-weight: 900; }
  .online { color: #34d399; }
  .cli-terminal { height: 90%; overflow-y: auto; background: #000; border: 1px solid #1e293b; padding: 1rem; }
  .log-line { margin: 0; font-size: 12px; }
  .idle { opacity: 0.3; }
</style>
