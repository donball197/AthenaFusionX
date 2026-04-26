<script>
    import { layout, systemVitals } from '$lib/core/store.svelte';
    import { fly } from 'svelte/transition';
    import { TaskService } from '$lib/services/TaskService';
    import SoundFx from '$lib/SoundFx';

    let stressActive = $state(false);

    function toggleStress() {
        SoundFx.click();
        stressActive = !stressActive;
        const directive = stressActive 
            ? "Execute SetStressTest with enabled=true. This is a system-wide thermal test."
            : "Disable stress test mode.";
        TaskService.runDirective(directive, 'TACTICAL_OVERRIDE');
    }

    function toggleVisor() {
        SoundFx.click();
        layout.visorVisible = !layout.visorVisible;
    }
</script>

{#if layout.explorerVisible}
<div class="tactical-wrap" transition:fly={{ x: -20, duration: 200 }}>
    <div class="inner glass-panel">
        <div class="blade-label">TACTICAL_UNITS</div>
        
        <button class="t-btn" class:is-on={layout.visorVisible} onclick={toggleVisor} title="Neural Visor">
            <span class="icon">👁</span>
            <span class="text">VISOR</span>
        </button>

        <button class="t-btn" class:is-on={stressActive} onclick={toggleStress} title="Stress Mode">
            <span class="icon">{stressActive ? '🔥' : '❄️'}</span>
            <span class="text">STRESS</span>
        </button>

        <div class="divider"></div>

        <div class="mini-graph" title="CPU Saturation">
            <div class="bar" style="height: {systemVitals.cpu_load * 100}%" class:hot={systemVitals.cpu_load > 0.8}></div>
        </div>
        
        <div class="thermal-dot" class:is-hot={systemVitals.temperature > 60}></div>
    </div>
</div>
{/if}

<style>
    .tactical-wrap { position: absolute; left: 224px; top: 50px; bottom: 30px; width: 45px; z-index: 40; }
    .inner { height: 100%; display: flex; flex-direction: column; align-items: center; padding: 15px 0; gap: 20px; border-radius: 0 8px 8px 0; background: rgba(2,2,10,0.8); border: 1px solid rgba(0,255,255,0.1); border-left: none; backdrop-filter: blur(10px); }
    .blade-label { writing-mode: vertical-rl; font-size: 7px; font-weight: 900; color: rgba(255,255,255,0.2); letter-spacing: 3px; margin-bottom: 10px; }
    .t-btn { background: none; border: none; cursor: pointer; display: flex; flex-direction: column; align-items: center; gap: 4px; opacity: 0.4; transition: 0.2s; color: white; }
    .t-btn.is-on { opacity: 1; filter: drop-shadow(0 0 5px #00ffff); color: #00ffff; }
    .t-btn .icon { font-size: 14px; }
    .t-btn .text { font-size: 6px; font-weight: 900; }
    .divider { width: 20px; height: 1px; background: rgba(255,255,255,0.1); }
    .mini-graph { width: 4px; height: 60px; background: rgba(255,255,255,0.05); position: relative; border-radius: 2px; }
    .mini-graph .bar { position: absolute; bottom: 0; width: 100%; background: #00ffff; transition: height 0.3s; border-radius: 2px; }
    .mini-graph .bar.hot { background: #f87171; box-shadow: 0 0 8px #f87171; }
    .thermal-dot { width: 6px; height: 6px; border-radius: 50%; background: #34d399; margin-top: auto; }
    .thermal-dot.is-hot { background: #f87171; box-shadow: 0 0 8px #f87171; animation: pulse 0.5s infinite alternate; }
    @keyframes pulse { from { opacity: 1; } to { opacity: 0.3; } }
</style>
