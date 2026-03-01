<script>
    export let resonance = 0.0;

    // Slower pulse = much less CPU/GPU usage
    $: pulseDuration = resonance > 0.6 ? '1.4s' : '3.2s';
    // Cap glow to prevent expensive filter rendering
    $: glow = Math.min(resonance * 10, 10);
</script>

<div class="flex flex-col gap-1 p-1.5 bg-black/30 border border-green-900/25 rounded shadow-inner">
    <div class="flex justify-between items-center text-[7px] text-green-800 uppercase tracking-tight">
        <span>Resonance Sync</span>
        <span class="text-blue-300 font-mono">{(resonance * 100).toFixed(0)}%</span>
    </div>

    <div class="h-8 w-full relative bg-[#020202] rounded-sm border border-green-900/15 overflow-hidden">
        <svg viewBox="0 0 100 20" class="w-full h-full">
            <path
                d="M0 10 L25 10 L30 3 L40 17 L50 10 L100 10"
                fill="none"
                stroke="currentColor"
                stroke-width="1"
                class="text-green-600 transition-all duration-700 ease-in-out"
                style="filter: drop-shadow(0 0 {glow}px rgba(34,197,94,0.35));"
            />
        </svg>

        <div
            class="absolute inset-0 bg-gradient-to-r from-transparent via-green-500/12 to-transparent w-1/5 h-full"
            style="animation: scan {pulseDuration} linear infinite;"
        ></div>
    </div>
</div>

<style>
    @keyframes scan {
        from { transform: translateX(-150%); }
        to   { transform: translateX(550%); }
    }
</style>
