<script>
    export let agents = []; // Array of 68 agent states
</script>

<div class="swarm-grid">
    {#each Array(68) as _, i}
        <div class="agent-dot" class:active={agents[i]}></div>
    {/each}
</div>

<style>
    .swarm-grid { display: grid; grid-template-columns: repeat(17, 1fr); gap: 4px; padding: 10px; }
    .agent-dot { width: 6px; height: 6px; background: #222; border-radius: 50%; }
    .agent-dot.active { background: #0f0; box-shadow: 0 0 5px #0f0; }
</style>
