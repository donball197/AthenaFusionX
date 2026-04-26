<script>
    import { systemVitals, vmoTelemetry } from '$lib/core/store.svelte';
    import { fly } from 'svelte/transition';

    let resFreq = $derived((systemVitals.resonance * 1000).toFixed(0));
    let pulseSpeed = $derived(2 - (systemVitals.resonance * 1.5));
</script>

<div class="mesh-v16">
    <div class="resonance-ring" style="animation-duration: {pulseSpeed}s">
        <div class="core-data">
            <span class="freq">{resFreq}Hz</span>
            <span class="label">NEURAL_RESONANCE</span>
        </div>
    </div>
    
    <div class="stats-overlay">
        <div class="stat">VMO_DENSITY: {vmoTelemetry.active_vmos}</div>
        <div class="stat">THERMAL_BIAS: {systemVitals.temperature.toFixed(1)}°C</div>
    </div>
</div>

<style>
    .mesh-v16 { height: 100%; display: flex; align-items: center; justify-content: center; background: radial-gradient(circle, #050a15 0%, #000 100%); position: relative; overflow: hidden; }
    .resonance-ring { width: 200px; height: 200px; border: 2px solid #00ffff; border-radius: 50%; display: flex; align-items: center; justify-content: center; box-shadow: 0 0 30px rgba(0,255,255,0.2); animation: pulse 2s infinite ease-in-out; }
    .core-data { text-align: center; font-family: 'JetBrains Mono', monospace; }
    .freq { display: block; font-size: 24px; font-weight: 900; color: #00ffff; }
    .label { font-size: 8px; color: rgba(0,255,255,0.4); letter-spacing: 2px; }
    .stats-overlay { position: absolute; bottom: 20px; left: 20px; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #34d399; }
    @keyframes pulse { 0%, 100% { transform: scale(1); opacity: 0.5; box-shadow: 0 0 20px rgba(0,255,255,0.2); } 50% { transform: scale(1.1); opacity: 1; box-shadow: 0 0 50px rgba(0,255,255,0.5); } }
</style>
