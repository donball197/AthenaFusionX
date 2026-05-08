<script>
    import { systemVitals } from '$lib/core/store.svelte';
    let { resonance = 0.5 } = $props();
    
    // Scale pulse speed based on CPU load (0.0 to 1.0)
    let pulseSpeed = $derived(0.5 + (systemVitals.cpu_load * 2.5));
</script>

<div class="heartbeat-container">
    <svg viewBox="0 0 100 40" class="waveform">
        <path 
            d="M0 20 L30 20 L35 10 L45 30 L50 20 L100 20" 
            fill="none" 
            stroke="var(--pulse-color, #00ffff)" 
            stroke-width="1.5"
            style="--speed: {pulseSpeed}s"
            class="pulse-path"
        />
    </svg>
    <div class="resonance-label">NEURAL_RESONANCE: {(resonance * 1000).toFixed(0)}Hz</div>
</div>

<style>
    .heartbeat-container { padding: 10px; background: rgba(0,0,0,0.3); border-radius: 4px; }
    .waveform { width: 100%; height: 40px; filter: drop-shadow(0 0 5px #00ffff); }
    .pulse-path {
        stroke-dasharray: 1000;
        stroke-dashoffset: 1000;
        animation: dash var(--speed) linear infinite;
    }
    @keyframes dash { to { stroke-dashoffset: 0; } }
    .resonance-label { 
        font-size: 8px; color: #00ffff; text-align: center; 
        font-family: 'JetBrains Mono', monospace; margin-top: 5px; opacity: 0.7;
    }
</style>
