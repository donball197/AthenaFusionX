<script>
    import { systemVitals } from '$lib/core/store.svelte.js';
    
    let cpuPercent = $derived((systemVitals.cpu_load * 100).toFixed(1));
    let ramVal = $derived(systemVitals.ram_gb || 0);
</script>

<div class="vitals-grid">
    <div class="stat-card">
        <div class="label">CPU_COMPUTE</div>
        <div class="value">{cpuPercent}%</div>
        <div class="bar-bg"><div class="bar-fill" style="width: {cpuPercent}%"></div></div>
    </div>
    
    <div class="stat-card">
        <div class="label">RAM_RESIDENT</div>
        <div class="value">{ramVal.toFixed(2)} GB</div>
        <div class="bar-bg"><div class="bar-fill" style="width: {(ramVal/4)*100}%"></div></div>
    </div>

    <div class="stat-card" class:warning={systemVitals.temperature > 60}>
        <div class="label">THERMAL_THRESHOLD</div>
        <div class="value">{systemVitals.temperature.toFixed(1)}°C</div>
    </div>
</div>

<style>
    .vitals-grid { display: grid; gap: 15px; }
    .stat-card { border: 1px solid rgba(0,255,255,0.1); padding: 10px; background: #050505; }
    .label { font-size: 8px; color: rgba(255,255,255,0.4); margin-bottom: 4px; }
    .value { font-size: 18px; font-weight: 900; color: #00ffff; }
    .bar-bg { height: 2px; background: rgba(255,255,255,0.05); margin-top: 8px; overflow: hidden; }
    .bar-fill { height: 100%; background: #00ffff; box-shadow: 0 0 10px #00ffff; transition: width 0.3s; }
    .warning .value { color: #f87171; text-shadow: 0 0 10px #f87171; }
</style>
