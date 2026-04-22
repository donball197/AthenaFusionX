<script>
    // ResonanceMeter.svelte — v2.0
    // FIX: Removed standalone WebSocket — was a 3rd concurrent connection
    //      to /ws/telemetry alongside NexusWS.js and nexus_socket.svelte.ts.
    //      Now reads from the shared reactive store.
    import { systemVitals, vmoTelemetry } from '$lib/core/store.svelte.js';

    let resonance  = $derived(systemVitals.resonance  || 0);
    let activeVmos = $derived(vmoTelemetry.active_vmos || 0);
    let temp       = $derived(systemVitals.temperature || 0);

    // SVG arc helper — circumference of r=140 is 879.6
    const C = 2 * Math.PI * 140;
    let dashOffset = $derived(C * (1 - Math.min(Math.max(resonance, 0), 0.9999)));
    let hue        = $derived((resonance * 120) % 360);
</script>

<div class="resonance-meter">
    <svg width="320" height="320" viewBox="0 0 320 320">
        <!-- Track -->
        <circle cx="160" cy="160" r="140"
            fill="none" stroke="#1e293b" stroke-width="25"/>
        <!-- Arc -->
        <circle cx="160" cy="160" r="140"
            fill="none"
            stroke="hsl({hue}, 100%, 65%)"
            stroke-width="25"
            stroke-dasharray="{C}"
            stroke-dashoffset="{dashOffset}"
            transform="rotate(-90 160 160)"
            style="transition: stroke-dashoffset 0.6s ease, stroke 0.6s ease;"/>
        <!-- Value -->
        <text x="160" y="155" text-anchor="middle"
              font-size="42" fill="#0ff" font-family="monospace" font-weight="900">
            {Math.round(resonance * 100)}%
        </text>
        <text x="160" y="190" text-anchor="middle"
              font-size="16" fill="#0ff" opacity="0.5" font-family="monospace">
            RESONANCE
        </text>
    </svg>

    <div class="stats">
        <div>VMOs: <span class="value">{activeVmos}</span></div>
        <div>Temp: <span class="value">{temp.toFixed(1)}°C</span></div>
    </div>
</div>

<style>
    .resonance-meter { text-align: center; margin: 2rem auto; }
    .stats {
        display: flex; justify-content: center;
        gap: 2rem; margin-top: 1rem;
        font-family: monospace; font-size: 14px; color: rgba(255,255,255,.5);
    }
    .value { color: #0ff; font-weight: bold; }
</style>
