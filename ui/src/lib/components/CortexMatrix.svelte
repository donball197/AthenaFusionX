<script lang="ts">
  import { telemetry } from '../core/telemetry.svelte.js';
</script>

<div class="matrix-root">
  <div class="matrix-header">
    CORTEX MATRIX // GRAPH OF THOUGHTS
  </div>
  
  <div class="nodes-container">
    {#if telemetry.graphNodes.length === 0}
      <div class="idle-text">No active cognitive graphs. Awaiting /cortex directive...</div>
    {/if}
    
    {#each telemetry.graphNodes as node}
      <div class="node-card" class:hypothesis={node.kind === 'Hypothesis'} class:observation={node.kind === 'Observation'}>
        <div class="node-type">{node.kind}</div>
        <div class="node-belief">
          <div class="belief-bar" style="width: {node.belief * 100}%"></div>
          <span>Belief: {(node.belief * 100).toFixed(1)}%</span>
        </div>
        <div class="node-content">{node.content}</div>
      </div>
    {/each}
  </div>
</div>

<style>
  .matrix-root {
    background: #050510;
    border: 1px solid #c084fc;
    border-radius: 4px;
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
  .matrix-header {
    background: rgba(192, 132, 252, 0.15);
    color: #c084fc;
    padding: 0.5rem 1rem;
    font-size: 12px;
    font-weight: bold;
    letter-spacing: 2px;
    border-bottom: 1px solid rgba(192, 132, 252, 0.5);
  }
  .nodes-container {
    padding: 1rem;
    overflow-y: auto;
    flex-grow: 1;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .idle-text {
    color: #475569;
    font-size: 13px;
    text-align: center;
    margin-top: 2rem;
    font-family: 'JetBrains Mono', monospace;
  }
  .node-card {
    background: #1e1e2f;
    border: 1px solid #333;
    border-left: 4px solid #fff;
    padding: 0.75rem;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
  }
  .hypothesis { border-left-color: #f59e0b; }
  .observation { border-left-color: #3b82f6; }
  
  .node-type {
    color: #94a3b8;
    margin-bottom: 0.5rem;
    text-transform: uppercase;
  }
  .node-content {
    color: #e2e8f0;
    white-space: pre-wrap;
    word-break: break-word;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .node-belief {
    margin-bottom: 0.5rem;
    background: #0f0f1a;
    height: 14px;
    position: relative;
    display: flex;
    align-items: center;
    padding: 0 5px;
    font-size: 10px;
    color: #fff;
    z-index: 1;
  }
  .belief-bar {
    position: absolute;
    left: 0; top: 0; bottom: 0;
    background: rgba(245, 158, 11, 0.3);
    z-index: -1;
    transition: width 0.5s ease-out;
  }
</style>
