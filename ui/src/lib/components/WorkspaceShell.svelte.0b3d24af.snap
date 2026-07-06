<script lang="ts">
  import { telemetry } from '../core/telemetry.svelte.js';
  import { uplink } from '../services/uplink.js';
  import CortexMatrix from './CortexMatrix.svelte';
  import { tick } from 'svelte';

  let command = $state("");
  let terminalDiv: HTMLElement;

  async function submitCommand(e: Event) {
    e.preventDefault();
    if (!command.trim()) return;

    const currentCmd = command;
    command = ""; 
    
    telemetry.logs = [...telemetry.logs, `\n> ${currentCmd}`];
    
    try {
        await uplink.post('/api/task', { directive: currentCmd });
    } catch (err) {
        telemetry.logs = [...telemetry.logs, `[NETWORK FAULT] Failed to send directive.`];
    }
    
    await tick();
    if (terminalDiv) {
        terminalDiv.scrollTop = terminalDiv.scrollHeight;
    }
  }
</script>

<div class="shell-layout">
  <div class="cli-root">
    <header class="cli-header">
      <div class="status-indicator" class:online={telemetry.connected}>
        {telemetry.connected ? '● UPLINK_STABLE' : '○ UPLINK_SEVERED'}
      </div>
      <div class="vitals">
        CPU: {(telemetry.vitals.cpu * 100).toFixed(1)}% | 
        RAM: {telemetry.vitals.ram.toFixed(2)}GB | 
        TEMP: {telemetry.vitals.temp.toFixed(1)}°C
      </div>
    </header>

    <div class="cli-terminal" bind:this={terminalDiv}>
      {#each telemetry.logs as log}
        <pre class="log-line">{log}</pre>
      {:else}
        <div class="idle">AWAITING_SUBSTRATE_DATA...</div>
      {/each}
    </div>

    <form class="cli-input-area" onsubmit={submitCommand}>
      <span class="prompt">nexus></span>
      <input 
        type="text" 
        bind:value={command} 
        placeholder="Enter directive (use /cortex <mission> for Graph of Thoughts)..." 
        autocomplete="off"
        spellcheck="false"
        disabled={!telemetry.connected}
      />
    </form>
  </div>
  
  <div class="matrix-panel">
    <CortexMatrix />
  </div>
</div>

<style>
  .shell-layout {
    display: flex;
    height: 100vh;
    background: #010105;
    width: 100vw;
    box-sizing: border-box;
  }
  .cli-root { 
    flex: 2;
    font-family: 'JetBrains Mono', monospace; 
    display: flex;
    flex-direction: column;
    padding: 1rem; 
    color: #00ffff; 
    border-right: 1px solid #1e293b;
  }
  .matrix-panel {
    flex: 1;
    padding: 1rem;
  }
  .cli-header {
    display: flex;
    justify-content: space-between;
    padding-bottom: 1rem;
    border-bottom: 1px solid #1e293b;
    margin-bottom: 1rem;
  }
  .status-indicator { font-size: 12px; font-weight: 900; }
  .vitals { font-size: 12px; color: #94a3b8; }
  .online { color: #34d399; }
  
  .cli-terminal { 
    flex-grow: 1; 
    overflow-y: auto; 
    background: #000; 
    border: 1px solid #1e293b; 
    padding: 1rem; 
    margin-bottom: 1rem;
  }
  .log-line { margin: 0; font-size: 13px; white-space: pre-wrap; word-wrap: break-word; }
  .idle { opacity: 0.3; }

  .cli-input-area {
    display: flex;
    align-items: center;
    background: #000;
    border: 1px solid #34d399;
    padding: 0.5rem 1rem;
  }
  .prompt { color: #34d399; font-weight: bold; margin-right: 0.5rem; }
  input {
    flex-grow: 1;
    background: transparent;
    border: none;
    color: #fff;
    font-family: 'JetBrains Mono', monospace;
    font-size: 14px;
    outline: none;
  }
  input:disabled { opacity: 0.5; }
</style>
