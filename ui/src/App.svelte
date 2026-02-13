<script>
    import { onMount } from 'svelte';
    import HiveCard from './lib/HiveCard.svelte';
    import AgentMap from './lib/AgentMap.svelte';
    import { initAudio, playResonance } from './lib/ResonatorAudio';

    let swarm = [];
    let ramUsage = "0MB";
    let status = "IDLE";
    let integrity = 0; // 0 to 100%

    async function fetchVitals() {
        try {
            const res = await fetch('/api/vitals');
            const data = await res.json();
            swarm = data.swarm || [];
            ramUsage = data.ram_usage || "0MB";
            status = data.status || "IDLE";
            
            // Logic: Integrity increases as agents stay active
            let active = swarm.filter(a => a.is_active).length;
            if (active > 0 && integrity < 100) {
                integrity = Math.min(100, integrity + (active * 0.05));
            }

            playResonance(ramUsage);
        } catch (e) { status = "OFFLINE"; }
    }

    onMount(() => {
        initAudio();
        const i = setInterval(fetchVitals, 2000);
        return () => clearInterval(i);
    });

    $: hives = Array.from({ length: 17 }, (_, i) => ({
        id: i,
        agents: swarm.filter(a => a.hive === i)
    }));
</script>

<main class="ide">
    <header>
        <div class="brand">NEXUS_V12 // {status}</div>
        <div class="vitals">{ramUsage}</div>
    </header>

    <div class="integrity-container">
        <div class="integrity-label">VAULT_INTEGRITY: {integrity.toFixed(1)}%</div>
        <div class="progress-track">
            <div class="progress-fill" style="width: {integrity}%"></div>
        </div>
    </div>

    <div class="command-bar">
        <button class="wake-btn" on:click={() => fetch('/api/wake', {method:'POST'})}>WAKE_ALL</button>
        <button class="flush-btn" on:click={() => fetch('/api/flush', {method:'POST'})}>KILL_ALL</button>
    </div>

    <AgentMap {swarm} />

    <div class="hive-container">
        {#each hives as hive}
            <HiveCard {...hive} />
        {/each}
    </div>
</main>

<style>
    :global(body) { margin: 0; background: #000; color: #0f0; font-family: monospace; overflow: hidden; }
    .ide { display: flex; flex-direction: column; height: 100vh; }
    header { padding: 8px; border-bottom: 1px solid #111; display: flex; justify-content: space-between; font-size: 10px; }
    
    .integrity-container { padding: 10px; background: #050505; border-bottom: 1px solid #111; }
    .integrity-label { font-size: 8px; color: #444; margin-bottom: 5px; }
    .progress-track { height: 4px; background: #111; width: 100%; overflow: hidden; }
    .progress-fill { height: 100%; background: #0f0; box-shadow: 0 0 10px #0f0; transition: width 0.5s ease; }

    .command-bar { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 10px; }
    button { background: #000; color: #0f0; border: 1px solid #0f0; font-size: 10px; padding: 12px; cursor: pointer; font-weight: bold; }
    .flush-btn { color: #f00; border-color: #f00; }
    
    .hive-container { flex-grow: 1; overflow-y: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(80px, 1fr)); gap: 10px; padding: 10px; }
</style>
