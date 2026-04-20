<script>
    import { neuralInventory, vmoTelemetry } from '$lib/core/store.svelte.js';
    import { fly } from 'svelte/transition';

    // Link physical VMO handles to the Neural Marketplace
    let activeNodes = $derived(Array.from({length: vmoTelemetry.active_vmos || 4}, (_, i) => ({
        id: i,
        handle: `0x${(200 + i).toString(16)}`,
        model: neuralInventory.models[i % neuralInventory.models.length],
        progress: 30 + (Math.random() * 60)
    })));
</script>

<div class="factory-grid">
    {#each activeNodes as node (node.id)}
        <div class="agent-card glass-panel" in:fly={{ y: 20, delay: node.id * 50 }}>
            <div class="card-hdr">
                <span class="status-pulse"></span>
                <span class="handle-lbl">{node.handle}</span>
                <span class="tier-badge">{node.model.tier}</span>
            </div>
            
            <div class="card-body">
                <div class="spec">
                    <span class="label">CORE:</span>
                    <span class="val text-cyan-400">{node.model.name}</span>
                </div>
                <div class="spec">
                    <span class="label">MISSION:</span>
                    <span class="val text-amber-400">{node.model.task}</span>
                </div>
                
                <div class="load-bar">
                    <div class="fill" style="width: {node.progress}%"></div>
                </div>
            </div>

            <div class="card-footer">
                <span class="uptime">SUBSTRATE_LINK_STABLE</span>
                <span class="percent">{(node.progress).toFixed(0)}%</span>
            </div>
        </div>
    {/each}
</div>

<style>
    .factory-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
        gap: 16px; padding: 10px;
    }
    .agent-card {
        background: rgba(2, 2, 15, 0.8);
        border: 1px solid rgba(0, 255, 255, 0.1);
        padding: 12px;
        position: relative;
        overflow: hidden;
    }
    .card-hdr { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; border-bottom: 1px solid rgba(255,255,255,0.05); padding-bottom: 8px; }
    .status-pulse { width: 6px; height: 6px; background: #34d399; border-radius: 50%; box-shadow: 0 0 8px #34d399; animation: blink 2s infinite; }
    .handle-lbl { font-size: 10px; font-weight: 900; color: #64748b; flex: 1; }
    .tier-badge { font-size: 7px; background: rgba(0, 255, 255, 0.1); color: #00ffff; padding: 2px 5px; border-radius: 2px; }
    
    .spec { display: flex; justify-content: space-between; font-size: 9px; margin-bottom: 4px; }
    .label { color: rgba(255,255,255,0.3); font-weight: 900; }
    .val { font-weight: 700; }

    .load-bar { height: 2px; background: rgba(255,255,255,0.05); margin-top: 10px; border-radius: 1px; overflow: hidden; }
    .fill { height: 100%; background: linear-gradient(90deg, #00ffff, #a78bfa); transition: width 0.5s; }

    .card-footer { display: flex; justify-content: space-between; margin-top: 10px; font-size: 7px; color: #475569; font-weight: 900; }
    @keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
</style>
