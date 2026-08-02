<script lang="ts">
  import { telemetry } from '../core/telemetry.svelte.js';
  import { uplink } from '../services/uplink.js';
  import { tick } from 'svelte';
  import Editor from './Editor.svelte';

  let command = $state("");
  let terminalDiv: HTMLElement;
  
  // Editor State
  let editorContent = $state("// Awaiting file selection...\n// The Polyglot AST engine mapped 40,000+ nodes.\n// Ready to load substrate...");
  let editorLang = $state("rust");
  let currentFile = $state("nexus_runtime_core/src/main.rs");

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

  $effect(() => {
    if (telemetry.logs && terminalDiv) {
      terminalDiv.scrollTop = terminalDiv.scrollHeight;
    }
  });
</script>

<div class="glass-root">
  
  <!-- Top Left: CodeMirror Editor -->
  <div class="glass-panel editor-panel">
    <header class="panel-header">
      <div class="brand-group">
        <div class="status-dot" class:online={telemetry.connected}></div>
        <span class="brand-text">ATHENA<span class="brand-highlight">FUSION</span></span>
      </div>
      <div class="file-info">{currentFile}</div>
    </header>
    <div class="editor-wrapper">
      <Editor bind:content={editorContent} language={editorLang} />
    </div>
  </div>

  <!-- Bottom Left: Terminal -->
  <div class="glass-panel terminal-panel">
    <header class="panel-header terminal-header">
      <span class="panel-title-small">UPLINK // TERMINAL</span>
      <div class="vitals">
        <span class="vital-metric">CPU <strong>{(telemetry.vitals.cpu * 100).toFixed(1)}%</strong></span>
        <span class="vital-metric">RAM <strong>{telemetry.vitals.ram.toFixed(2)}GB</strong></span>
        <span class="vital-metric">TMP <strong>{telemetry.vitals.temp.toFixed(1)}°C</strong></span>
      </div>
    </header>

    <div class="terminal-container">
      <div class="terminal-feed" bind:this={terminalDiv}>
        {#each telemetry.logs as log}
          <pre class="log-line" 
               class:error={log.includes('FAULT') || log.includes('FAILED') || log.includes('VETO')} 
               class:event={log.includes('[EVENT]') || log.includes('>>')}
               class:success={log.includes('[OK]') || log.includes('PROVEN')}>
            {log}
          </pre>
        {:else}
          <div class="idle-text">Substrate online. Awaiting neural uplink data...</div>
        {/each}
      </div>
      <div class="scanlines"></div>
    </div>

    <div class="input-wrapper">
      <form class="cli-input-bubble" onsubmit={submitCommand}>
        <span class="prompt">nexus <span class="chevron">❯</span></span>
        <input 
          type="text" 
          bind:value={command} 
          placeholder="Enter /cortex <mission> or a system directive..." 
          autocomplete="off"
          spellcheck="false"
          disabled={!telemetry.connected}
        />
        <button class="exec-btn" type="submit" aria-label="Execute directive" disabled={!command.trim() || !telemetry.connected}>
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"></line><polygon points="22 2 15 22 11 13 2 9 22 2"></polygon></svg>
        </button>
      </form>
    </div>
  </div>

  <!-- Right Side: Analytics & Swarm Vis -->
  <div class="sidebar">
    <div class="glass-panel dag-panel">
      <h3 class="panel-title">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" style="margin-right: 6px;"><circle cx="18" cy="5" r="3"></circle><circle cx="6" cy="12" r="3"></circle><circle cx="18" cy="19" r="3"></circle><line x1="8.59" y1="13.51" x2="15.42" y2="17.49"></line><line x1="15.41" y1="6.51" x2="8.59" y2="10.49"></line></svg>
        GRAPH OF THOUGHTS
      </h3>
      <div class="dag-list">
        {#each Object.values(telemetry.dagNodes) as node}
          <div class="dag-node status-{node.status}">
            <div class="node-header">
              <span class="node-id">{node.id.split('-')[0]}</span>
              <span class="node-status-badge">{node.status.toUpperCase()}</span>
            </div>
            <div class="node-obj">{node.objective}</div>
          </div>
        {/each}
        {#if Object.keys(telemetry.dagNodes).length === 0}
          <div class="idle-state">
            <div class="pulse-ring"></div>
            <p>No active cognitive graphs.</p>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  :global(body) { 
    background-color: #030305;
    background-image: 
      linear-gradient(rgba(52, 211, 153, 0.03) 1px, transparent 1px),
      linear-gradient(90deg, rgba(52, 211, 153, 0.03) 1px, transparent 1px),
      radial-gradient(circle at 100% 0%, rgba(52, 211, 153, 0.08), transparent 50%),
      radial-gradient(circle at 0% 100%, rgba(192, 132, 252, 0.08), transparent 50%);
    background-size: 30px 30px, 30px 30px, 100% 100%, 100% 100%;
    margin: 0; 
  }
  
  .glass-root { 
    font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace; 
    display: grid;
    /* IDE 3-Pane Layout */
    grid-template-columns: 1fr 380px;
    grid-template-rows: minmax(0, 2fr) minmax(0, 1fr);
    grid-template-areas: 
      "editor sidebar"
      "terminal sidebar";
    gap: 1.5rem;
    height: 100vh;
    padding: 1.5rem; 
    box-sizing: border-box;
  }

  .editor-panel { grid-area: editor; }
  .terminal-panel { grid-area: terminal; }
  .sidebar { grid-area: sidebar; display: flex; flex-direction: column; gap: 1.5rem; overflow: hidden; }

  .glass-panel {
    background: rgba(10, 12, 20, 0.65);
    border: 1px solid rgba(52, 211, 153, 0.15);
    border-radius: 16px;
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255,255,255,0.05);
  }

  /* EDITOR SPECIFIC */
  .editor-wrapper {
    flex-grow: 1;
    overflow: hidden;
    padding: 0.5rem;
  }
  
  .file-info {
    font-size: 12px;
    color: #64748b;
    background: rgba(0,0,0,0.3);
    padding: 4px 12px;
    border-radius: 4px;
    border: 1px solid rgba(255,255,255,0.05);
  }

  /* HEADER STYLING */
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid rgba(52, 211, 153, 0.15);
    background: linear-gradient(to right, rgba(0, 0, 0, 0.4), rgba(0, 0, 0, 0.1));
  }

  .terminal-header {
    padding: 0.5rem 1.5rem;
  }

  .brand-group {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #475569;
    box-shadow: 0 0 0 rgba(0,0,0,0);
  }
  .status-dot.online {
    background: #10b981;
    box-shadow: 0 0 10px #10b981, 0 0 20px #10b981;
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0% { box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.4); }
    70% { box-shadow: 0 0 0 6px rgba(16, 185, 129, 0); }
    100% { box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
  }

  .brand-text {
    font-size: 14px;
    font-weight: 800;
    letter-spacing: 2px;
    color: #94a3b8;
  }
  .brand-highlight { color: #34d399; }

  .vitals { 
    display: flex; 
    gap: 1.5rem; 
    font-size: 12px; 
    color: #64748b; 
  }
  .vital-metric strong {
    color: #e2e8f0;
    margin-left: 4px;
  }
  
  /* TERMINAL STYLING */
  .terminal-container {
    position: relative;
    flex-grow: 1;
    background: #020205;
    overflow: hidden;
  }

  .terminal-feed { 
    height: 100%;
    overflow-y: auto; 
    padding: 1.5rem; 
    color: #cbd5e1;
    box-sizing: border-box;
  }

  .scanlines {
    position: absolute;
    top: 0; left: 0; right: 0; bottom: 0;
    background: linear-gradient(to bottom, rgba(255,255,255,0), rgba(255,255,255,0) 50%, rgba(0,0,0,0.1) 50%, rgba(0,0,0,0.1));
    background-size: 100% 4px;
    pointer-events: none;
    z-index: 10;
  }

  .log-line { 
    margin: 0 0 6px 0; 
    font-size: 13.5px; 
    white-space: pre-wrap; 
    word-wrap: break-word; 
    line-height: 1.6; 
  }
  
  .log-line.event { color: #818cf8; }
  .log-line.error { color: #f87171; }
  .log-line.success { color: #34d399; }
  
  /* INPUT BUBBLE STYLING */
  .input-wrapper {
    padding: 1rem 1.5rem;
    background: linear-gradient(to top, rgba(10, 12, 20, 0.9), rgba(10, 12, 20, 0));
  }

  .cli-input-bubble {
    display: flex;
    align-items: center;
    background: rgba(2, 2, 5, 0.8);
    border: 1px solid rgba(52, 211, 153, 0.3);
    border-radius: 9999px;
    padding: 0.5rem 0.5rem 0.5rem 1.5rem;
    box-shadow: 0 8px 25px rgba(0, 0, 0, 0.6), inset 0 0 15px rgba(52, 211, 153, 0.05);
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .cli-input-bubble:focus-within {
    border-color: #34d399;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.8), 0 0 20px rgba(52, 211, 153, 0.2), inset 0 0 15px rgba(52, 211, 153, 0.1);
    transform: translateY(-2px);
  }

  .prompt { 
    color: #94a3b8; 
    font-weight: 700; 
    margin-right: 0.75rem; 
  }
  .chevron { color: #34d399; }

  input {
    flex-grow: 1;
    background: transparent;
    border: none;
    color: #f8fafc;
    font-family: 'JetBrains Mono', monospace;
    font-size: 14px;
    outline: none;
  }
  input::placeholder { color: #475569; }

  .exec-btn {
    background: #34d399;
    color: #020205;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    cursor: pointer;
    transition: all 0.2s;
  }

  .exec-btn:hover:not(:disabled) {
    background: #10b981;
    transform: scale(1.05);
    box-shadow: 0 0 15px rgba(16, 185, 129, 0.4);
  }

  .exec-btn:disabled {
    background: #334155;
    color: #64748b;
    cursor: not-allowed;
  }

  /* SIDEBAR PANELS */
  .panel-title {
    display: flex;
    align-items: center;
    margin: 0 0 1.5rem 0;
    font-size: 13px;
    color: #e2e8f0;
    letter-spacing: 0.1em;
    font-weight: 700;
  }
  
  .panel-title-small {
    font-size: 12px;
    color: #94a3b8;
    letter-spacing: 0.1em;
    font-weight: bold;
  }

  .dag-panel { padding: 1.5rem; flex-grow: 1; overflow-y: auto; }
  .dag-list { display: flex; flex-direction: column; gap: 1rem; }
  
  .dag-node {
    padding: 1.25rem;
    border-radius: 12px;
    border: 1px solid rgba(255,255,255,0.05);
    background: rgba(0,0,0,0.3);
    transition: all 0.3s ease;
  }
  
  .node-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.75rem;
  }
  
  .node-id { font-size: 11px; color: #94a3b8; font-family: monospace; }
  .node-status-badge { 
    font-size: 10px; 
    font-weight: 800; 
    padding: 2px 8px; 
    border-radius: 999px; 
    letter-spacing: 0.5px;
  }
  
  .node-obj { font-size: 13px; color: #e2e8f0; line-height: 1.5; }

  /* Node Status Colors */
  .status-Running { 
    border-color: rgba(96,165,250,0.4); 
    background: linear-gradient(135deg, rgba(96,165,250,0.1), rgba(0,0,0,0.3)); 
    box-shadow: 0 0 15px rgba(96,165,250,0.1);
  }
  .status-Running .node-status-badge { background: rgba(96,165,250,0.2); color: #93c5fd; }
  
  .status-Completed { 
    border-color: rgba(52,211,153,0.2); 
    opacity: 0.6; 
  }
  .status-Completed .node-status-badge { background: rgba(52,211,153,0.1); color: #34d399; }
  
  .status-Failed { 
    border-color: rgba(248,113,113,0.4); 
    background: rgba(248,113,113,0.05); 
  }
  .status-Failed .node-status-badge { background: rgba(248,113,113,0.2); color: #fca5a5; }

  .idle-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: #475569;
    font-style: italic;
    font-size: 13px;
  }

  .pulse-ring {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: 2px solid #334155;
    margin-bottom: 1rem;
    animation: slow-pulse 3s infinite ease-in-out;
  }

  @keyframes slow-pulse {
    0% { transform: scale(0.8); opacity: 0.5; }
    50% { transform: scale(1.2); opacity: 1; border-color: rgba(52, 211, 153, 0.3); }
    100% { transform: scale(0.8); opacity: 0.5; }
  }
</style>
