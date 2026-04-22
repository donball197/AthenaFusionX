<script>
    import { layout, systemVitals } from '$lib/core/store.svelte.js';
    import { fly } from 'svelte/transition';
    import SoundFx from '$lib/SoundFx.js';

    function toggle(key) {
        SoundFx.click();
        layout[key] = !layout[key];
    }
</script>

{#if layout.explorerVisible}
<div class="tactical-wrap" transition:fly={{ x: -20, duration: 200 }}>
    <div class="inner glass-panel">
        <div class="blade-label">TACTICAL_UNITS</div>
        
        <button class="t-btn" class:on={layout.visorVisible} onclick={() => toggle('visorVisible')}>
            <span class="icon">👁</span>
            <span class="text">VISOR</span>
        </button>

        <button class="t-btn" class:on={layout.autopilotActive} onclick={() => toggle('autopilotActive')}>
            <span class="icon">🤖</span>
            <span class="text">PILOT</span>
        </button>

        <div class="divider"></div>

        <div class="mini-graph">
            <div class="bar" style="height: {systemVitals.cpu_load * 100}%"></div>
            <div class="label">CPU</div>
        </div>
    </div>
</div>
{/if}

<style>
    .tactical-wrap { position: absolute; left: 220px; top: 50px; bottom: 30px; width: 45px; z-index: 40; }
    .inner { height: 100%; display: flex; flex-direction: column; align-items: center; padding: 15px 0; gap: 20px; border-radius: 0 8px 8px 0; border-left: none; }
    .blade-label { writing-mode: vertical-rl; font-size: 7px; font-weight: 900; color: rgba(255,255,255,0.2); letter-spacing: 3px; }
    .t-btn { background: none; border: none; cursor: pointer; display: flex; flex-direction: column; align-items: center; gap: 4px; opacity: 0.4; transition: 0.2s; }
    .t-btn.on { opacity: 1; filter: drop-shadow(0 0 5px #00ffff); }
    .t-btn .icon { font-size: 14px; }
    .t-btn .text { font-size: 6px; font-weight: 900; color: #00ffff; }
    .divider { width: 20px; height: 1px; background: rgba(255,255,255,0.1); }
    .mini-graph { width: 4px; height: 40px; background: rgba(255,255,255,0.05); position: relative; border-radius: 2px; }
    .mini-graph .bar { position: absolute; bottom: 0; width: 100%; background: #fb923c; transition: height 0.3s; border-radius: 2px; }
    .mini-graph .label { position: absolute; top: 100%; font-size: 5px; margin-top: 4px; }
</style>
