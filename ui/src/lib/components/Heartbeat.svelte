<script>
    export let resonance = 0.0;
    
    // Calculate pulse speed based on resonance (higher resonance = faster pulse)
    $: pulseDuration = resonance > 0.5 ? '0.8s' : '2s';
    $: glowIntensity = resonance * 20;
</script>

<div class="flex flex-col gap-2 p-2 bg-black/40 border border-green-900/20 rounded shadow-inner">
    <div class="flex justify-between items-center px-1">
        <span class="text-[8px] text-green-700 tracking-tighter uppercase font-bold">Resonance Sync</span>
        <span class="text-[9px] text-blue-400 font-mono">{(resonance * 100).toFixed(1)}%</span>
    </div>
    
    <div class="h-12 w-full flex items-center justify-center relative overflow-hidden bg-[#050505] rounded-sm border border-green-900/10">
        <svg viewBox="0 0 100 40" class="w-full h-full">
            <path
                d="M0 20 L30 20 L35 5 L45 35 L50 20 L100 20"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                class="text-green-500 transition-all duration-500"
                style="filter: drop-shadow(0 0 {glowIntensity}px rgba(34, 197, 94, 0.8));"
            />
        </svg>

        <div 
            class="absolute inset-0 bg-gradient-to-r from-transparent via-green-500/20 to-transparent w-1/4 h-full"
            style="animation: scan {pulseDuration} linear infinite;"
        ></div>
    </div>
</div>

<style>
    @keyframes scan {
        from { transform: translateX(-100%); }
        to { transform: translateX(400%); }
    }
</style>
