<script>
    let { resonance = 0.0 } = $props();

    let pulseDuration = $derived(resonance > 0.6 ? '0.6s' : resonance > 0.4 ? '1.1s' : '2.2s');
    let glowIntensity = $derived(Math.floor(resonance * 28));
</script>

<div class="bg-black/60 border border-green-900/30 p-4 rounded">
    <div class="flex justify-between text-[9px] uppercase tracking-widest mb-2">
        <span class="text-green-700">RESONANCE SYNC</span>
        <span class="font-mono text-blue-400">{(resonance * 100).toFixed(1)}%</span>
    </div>
    
    <div class="relative h-14 bg-[#0a0a0a] border border-green-900/20 rounded flex items-center justify-center overflow-hidden">
        <svg viewBox="0 0 180 50" class="w-full h-10">
            <path 
                d="M0 25 L35 25 L42 8 L55 42 L68 25 L110 25 L118 12 L130 38 L145 25 L180 25" 
                fill="none" 
                stroke="#22c55e" 
                stroke-width="2.5"
                style="filter: drop-shadow(0 0 {glowIntensity}px #22c55e);"
            />
        </svg>
        <div 
            class="absolute inset-0 bg-gradient-to-r from-transparent via-green-500/30 to-transparent"
            style="animation: scan {pulseDuration} linear infinite;"
        ></div>
    </div>
</div>

<style>
    @keyframes scan {
        from { transform: translateX(-150%); }
        to { transform: translateX(300%); }
    }
</style>
