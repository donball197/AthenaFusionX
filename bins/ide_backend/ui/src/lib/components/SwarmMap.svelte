<script>
    import { onMount, onDestroy } from 'svelte';
    import { fly } from 'svelte/transition';
    import { uplink } from '$lib/services/uplink.js';

    let nodes = $state([]);
    let pollTimer;

    async function fetchSwarm() {
        try {
            const r = await uplink.fetch('/api/cluster/nodes');
            if (r.ok) {
                const data = await r.json();
                nodes = data.nodes || [];
            }
        } catch (e) { console.error('Swarm sync failure'); }
    }

    onMount(() => {
        fetchSwarm();
        pollTimer = setInterval(fetchSwarm, 3000);
    });
    onDestroy(() => clearInterval(pollTimer));
</script>

<div class="swarm-container">
    <div class="swarm-hdr">◈ DISTRIBUTED_SWARM_TOPOLOGY</div>
    <div class="node-grid">
        {#each nodes as node (node.id)}
            <div class="node-card" in:fly={{ y: 10 }}>
                <div class="node-status" style="background: {node.cpu > 80 ? '#f87171' : '#34d399'}"></div>
                <div class="node-main">
                    <div class="node-id">{node.id}</div>
                    <div class="node-host">{node.host}:{node.port}</div>
                </div>
                <div class="node-stats">
                    <span>CPU: {node.cpu.toFixed(0)}%</span>
                    <span>TEMP: {node.temp.toFixed(1)}°C</span>
                </div>
            </div>
        {:else}
            <div class="swarm-empty">NO REMOTE SUBSTRATES DETECTED // STANDALONE_MODE</div>
        {/each}
    </div>
</div>

<style>
    .swarm-container { padding: 20px; background: #010108; height: 100%; font-family: monospace; }
    .swarm-hdr { font-size: 11px; font-weight: 900; color: #a78bfa; border-bottom: 1px solid rgba(167,139,250,0.2); padding-bottom: 10px; margin-bottom: 20px; }
    .node-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; }
    .node-card { background: rgba(167,139,250,0.05); border: 1px solid rgba(167,139,250,0.1); padding: 12px; display: flex; align-items: center; gap: 12px; }
    .node-status { width: 8px; height: 8px; border-radius: 50%; box-shadow: 0 0 8px currentColor; }
    .node-id { font-size: 10px; font-weight: bold; color: #e2e8f0; }
    .node-host { font-size: 8px; color: #64748b; }
    .node-stats { font-size: 8px; color: #a78bfa; display: flex; flex-direction: column; text-align: right; margin-left: auto; }
    .swarm-empty { grid-column: 1/-1; text-align: center; padding: 40px; color: #334155; font-size: 10px; letter-spacing: 2px; }
</style>
