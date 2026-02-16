<script>
    import { fly } from 'svelte/transition';
    export let isOpen = true;
    export let agents = Array(68).fill({status: 'IDLE'});
</script>

{#if isOpen}
<div class="sidebar" transition:fly={{ x: -300, duration: 300 }}>
    <div class="sidebar-header">SWARM_INDEX</div>
    <div class="agent-list">
        {#each agents as agent, i}
            <div class="agent-row">
                <span class="ln">{i + 1}</span>
                <span class="id">AGENT_{i.toString().padStart(2, '0')}</span>
                <span class="status {agent.status}">{agent.status}</span>
            </div>
        {/each}
    </div>
</div>
{/if}

<style>
    .sidebar { width: 250px; background: #050505; border-right: 1px solid #1a1a1a; height: 100vh; overflow-y: auto; }
    .sidebar-header { padding: 15px; font-size: 10px; color: #666; letter-spacing: 2px; border-bottom: 1px solid #1a1a1a; }
    .agent-row { display: flex; align-items: center; padding: 8px 15px; font-family: monospace; font-size: 12px; }
    .agent-row:hover { background: #0a0a0a; }
    .ln { width: 30px; color: #333; font-size: 10px; }
    .id { flex-grow: 1; color: #ccc; }
    .status { font-size: 9px; padding: 2px 4px; border-radius: 2px; }
    .IDLE { color: #444; }
    .ACTIVE { color: #0f0; background: #003300; }
</style>
