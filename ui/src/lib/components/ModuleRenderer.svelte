<script lang="ts">
    import { agentRegistry } from '../core/agent_module_registry.svelte.js';
    import { pushTermLine } from '../core/NexusWS.js';
    let { activeId } = $props<{ activeId: string }>();
    let m = $derived(agentRegistry.modules.find(x => x.id === activeId));

    async function fire(directive: string) {
        pushTermLine(`>> [MODULE_INTENT] ${directive}`, 'text-cyan-400');
        await fetch('/api/task', { method: 'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({directive}) });
    }
</script>

{#if m}
    <div class="h-full border border-cyan-500/20 bg-cyan-950/10 flex flex-col font-mono text-xs">
        <div class="p-2 border-b border-cyan-500/20 bg-black flex justify-between text-cyan-400">
            <b>{m.id}</b><span class="opacity-50">PROVENANCE: {m.generatedBy || 'UNKNOWN'}</span>
        </div>
        <div class="p-4 flex-1 overflow-auto text-gray-300 whitespace-pre-wrap">{m.render.source || `[${m.render.kind.toUpperCase()} MOCK]`}</div>
        {#if m.render.actions}
            <div class="p-3 border-t border-cyan-500/20 flex gap-2 flex-wrap">
                {#each m.render.actions as a}
                    <button onclick={() => fire(a.directive)} class="border border-cyan-500/30 px-3 py-1 hover:bg-cyan-500/20 text-cyan-400">{a.label}</button>
                {/each}
            </div>
        {/if}
    </div>
{:else}
    <div class="h-full flex items-center justify-center font-mono text-gray-700 tracking-widest">AWAITING DYNAMIC MODULE</div>
{/if}
