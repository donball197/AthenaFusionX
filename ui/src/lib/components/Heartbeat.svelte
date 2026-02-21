<script>
    export let resonance = 0.0;
    
    // Dynamic Animation based on Entropy
    // High Resonance = Fast Pulse, High Opacity
    $: pulseDuration = Math.max(0.5, 2 - (resonance * 1.5)); 
    $: intensity = Math.min(1.0, 0.2 + (resonance * 0.8));
</script>

<div class="flex flex-col items-center justify-center p-4 border-t border-[#1a1a1a] mt-4">
    <div 
        class="relative w-12 h-12 rounded-full bg-green-500 shadow-[0_0_15px_rgba(34,197,94,0.5)] flex items-center justify-center transition-all duration-500"
        style="
            animation: pulse {pulseDuration}s infinite ease-in-out;
            opacity: {intensity};
        "
    >
        <div class="absolute inset-0 rounded-full border border-green-300 animate-ping opacity-75"></div>
        <span class="text-black font-bold text-[8px] z-10 font-mono">GGUF</span>
    </div>
    
    <div class="flex flex-col items-center mt-3">
        <span class="text-[9px] text-green-500 font-bold uppercase tracking-widest">
            Resonance
        </span>
        <span class="text-[10px] font-mono text-white">
            {(resonance * 100).toFixed(2)}%
        </span>
    </div>
</div>

<style>
    @keyframes pulse {
        0%, 100% { transform: scale(0.95); box-shadow: 0 0 10px rgba(34,197,94,0.4); }
        50% { transform: scale(1.1); box-shadow: 0 0 25px rgba(34,197,94,0.9); }
    }
</style>
