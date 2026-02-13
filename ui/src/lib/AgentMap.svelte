<script>
    export let swarm = [];

    // Simple hash to map a filename to a consistent coordinate
    function getCoords(target) {
        if (!target || target === "IDLE" || target === "STANDBY") {
            return { x: 50, y: 50, active: false };
        }
        let hash = 0;
        for (let i = 0; i < target.length; i++) {
            hash = target.charCodeAt(i) + ((hash << 5) - hash);
        }
        return {
            x: Math.abs((hash % 80) + 10), // Keep within 10-90% range
            y: Math.abs(((hash >> 8) % 80) + 10),
            active: true
        };
    }
</script>

<div class="map-container">
    <div class="grid-overlay"></div>
    <div class="scanner-line"></div>
    
    {#each swarm as agent}
        {@const pos = getCoords(agent.target)}
        <div class="map-dot" 
             class:active={pos.active}
             style="left: {pos.x}%; top: {pos.y}%;">
            {#if pos.active}
                <div class="strike-ring"></div>
            {/if}
        </div>
    {/each}
</div>

<style>
    .map-container { position: relative; height: 350px; background: #000; border: 1px solid #1a1a1a; overflow: hidden; margin-bottom: 20px; }
    .grid-overlay { position: absolute; inset: 0; background-image: radial-gradient(#111 1px, transparent 1px); background-size: 30px 30px; }
    
    /* The "Radar" sweep effect */
    .scanner-line { position: absolute; width: 100%; height: 2px; background: linear-gradient(to bottom, transparent, #0f0, transparent); top: 0; opacity: 0.1; animation: scan 4s linear infinite; }
    @keyframes scan { from { top: 0; } to { top: 100%; } }

    .map-dot { position: absolute; width: 4px; height: 4px; background: #222; border-radius: 50%; transition: all 0.8s cubic-bezier(0.4, 0, 0.2, 1); }
    .map-dot.active { background: #0f0; box-shadow: 0 0 8px #0f0; z-index: 10; width: 6px; height: 6px; }

    .strike-ring { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); width: 20px; height: 20px; border: 1px solid #0f0; border-radius: 50%; opacity: 0; animation: strike 1.5s ease-out infinite; }
    @keyframes strike { 0% { width: 0; height: 0; opacity: 0.8; } 100% { width: 40px; height: 40px; opacity: 0; } }
</style>
