<script>
    import { layout } from "$lib/core/store.svelte.js";
    import { WS } from "$lib/core/NexusWS.js";
    let resonance = 50;

    function updateResonance() {
        if (WS && WS.readyState === 1) {
            WS.send(JSON.stringify({ type: "resonance_update", data: resonance }));
        }
    }
</script>

<footer class="status-bar">
    <div class="left">
        <span class="status-dot online"></span>
        <span>NEXUS v1.9.0 [ARM64_DNA]</span>
    </div>
    
    <div class="center">
        <label for="resonance">RESONANCE:</label>
        <input 
            type="range" 
            id="resonance" 
            min="0" max="100" 
            bind:value={resonance} 
            onchange={updateResonance}
        />
        <span class="val">{resonance}%</span>
    </div>

    <div class="right">
        <span>MEM: MIMALLOC_ACTIVE</span>
    </div>
</footer>

<style>
    .status-bar { display: flex; justify-content: space-between; align-items: center; background: #0d1117; padding: 0 12px; border-top: 1px solid #222; font-size: 10px; color: #8b949e; }
    .center { display: flex; align-items: center; gap: 10px; }
    input[type=range] { width: 100px; accent-color: #58a6ff; }
    .status-dot.online { width: 6px; height: 6px; background: #238636; border-radius: 50%; box-shadow: 0 0 5px #238636; }
</style>
