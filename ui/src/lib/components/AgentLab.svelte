<script>
    import { pushTermLine } from '$lib/core/NexusWS.js';
    import { vmoTelemetry } from '$lib/core/store.svelte.js';

    let missions = $state([]);
    let loading = $state(false);
    let directive = $state('');

    const AGENTS = Array.from({length: 12}, (_, i) => ({
        id: i,
        name: `DRONE_${String(i).padStart(2, '0')}`,
        status: i < 3 ? 'ACTIVE' : i < 6 ? 'STANDBY' : 'IDLE'
    }));

    async function broadcast() {
        const d = directive.trim();
        if (!d) return;
        pushTermLine(`>> [SWARM] Broadcasting: ${d}`, 'text-amber-400');
        directive = '';
        try {
            await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive: `[SWARM_TASK] ${d}` })
            });
        } catch (e) {
            pushTermLine(`!! [SWARM_FAULT]: ${e.message}`, 'text-rose-400');
        }
    }
</script>

<div class="swarm-lab h-full flex flex-col bg-[#02030a] font-mono">
    <div class="header flex justify-between items-center p-4 border-b border-amber-500/10">
        <div class="title text-amber-500 font-bold tracking-widest text-xs">SWARM_REFLEX_LAB</div>
        <div class="meta text-[8px] text-amber-900">NODE_COUNT: {AGENTS.length} | ACTIVE_VMO: {vmoTelemetry.active_vmos}</div>
    </div>
    
    <div class="flex-1 overflow-y-auto p-4 space-y-4">
        <div class="grid grid-cols-3 gap-2">
            {#each AGENTS as agent}
                <div class="agent-card p-2 border border-white/5 bg-black/40 flex items-center gap-2">
                    <div class="pulse-dot w-2 h-2 rounded-full" style="background: {agent.status === 'ACTIVE' ? '#34d399' : '#374151'}"></div>
                    <div class="text-[8px] text-gray-500">{agent.name}</div>
                </div>
            {/each}
        </div>

        <div class="input-zone border border-amber-500/20 p-4">
            <div class="text-[8px] text-amber-900 mb-2 underline">BROADCAST_DIRECTIVE</div>
            <div class="flex gap-2">
                <input bind:value={directive} onkeydown={(e) => e.key === 'Enter' && broadcast()} 
                       class="flex-1 bg-black border border-amber-500/30 text-amber-500 text-[10px] p-2 outline-none" />
                <button onclick={broadcast} class="bg-amber-900/20 border border-amber-500/40 text-amber-500 text-[8px] px-4 font-bold">SEND</button>
            </div>
        </div>
    </div>
</div>
