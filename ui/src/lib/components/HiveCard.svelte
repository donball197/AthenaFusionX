<script>
    import { createEventDispatcher } from 'svelte';
    const dispatch = createEventDispatcher();
    export let id;
    export let agents = [];
    export let isFocused = false;

    // Calculate heat based on how many agents are active in this hive
    $: activeCount = agents.filter(a => a.is_active).length;
    $: heatLevel = activeCount / 4; // 0.0 to 1.0
</script>

<div 
    class="card" 
    class:focused={isFocused} 
    style="--heat: {heatLevel}; --border-glow: {activeCount > 0 ? '#0f0' : '#111'}"
    on:click={() => dispatch('click')}
>
    <div class="card-header">
        <span>HIVE_{id.toString().padStart(2, '0')}</span>
        {#if isFocused}<span class="close">EXIT [X]</span>{/if}
    </div>

    <div class="agent-grid">
        {#each agents as agent}
            <div class="agent-node" class:active={agent.is_active}>
                {#if isFocused}<span class="agent-id">A_{agent.id}</span>{/if}
            </div>
        {/each}
    </div>

    {#if isFocused}
        <div class="focus-controls">
            <button class="strike-btn" on:click|stopPropagation={() => dispatch('strike')}>
                INITIATE_HIVE_STRIKE
            </button>
        </div>
    {/if}
</div>

<style>
    .card { 
        background: rgba(0, calc(var(--heat) * 40), 0, 0.2); 
        border: 1px solid var(--border-glow); 
        padding: 10px; 
        transition: all 0.5s ease; 
        cursor: pointer;
        box-shadow: inset 0 0 10px rgba(0, 255, 0, calc(var(--heat) * 0.3));
    }
    .focused { background: #000; border: 1px solid #0f0; height: 100%; box-shadow: 0 0 20px #0f0; }
    .card-header { font-size: 9px; color: #444; display: flex; justify-content: space-between; margin-bottom: 8px; }
    .agent-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; }
    .agent-node { height: 6px; background: #111; transition: background 0.3s; }
    .active { background: #0f0; box-shadow: 0 0 5px #0f0; }
    .focused .agent-node { height: 30px; border: 1px solid #222; }
    .strike-btn { width: 100%; background: #000; color: #0f0; border: 1px solid #0f0; padding: 10px; margin-top: 15px; font-family: monospace; }
</style>
