<script>
    export let resonance = 0.0;
    
    $: pulseDuration = Math.max(0.5, 2 - (resonance * 1.5)); 
    $: intensity = Math.min(1.0, 0.2 + (resonance * 0.8));
</script>

<div class="flex flex-col items-center justify-center p-2 border-t border-[#1a1a1a] mt-4">
    <div 
        class="relative w-8 h-8 rounded-full bg-green-500 shadow-[0_0_15px_rgba(34,197,94,0.5)] flex items-center justify-center"
        style="
            animation: pulse {pulseDuration}s infinite ease-in-out;
            opacity: {intensity};
        "
    >
        <div class="absolute inset-0 rounded-full border border-green-300 animate-ping opacity-75"></div>
        <span class="text-black font-bold text-[8px]">GGUF</span>
    </div>
    <span class="text-[8px] text-green-500 font-bold uppercase tracking-widest mt-2 text-center">
        Resonance<br/>{(resonance * 100).toFixed(1)}%
    </span>
</div>

<style>
    @keyframes pulse {
        0%, 100% { transform: scale(0.95); box-shadow: 0 0 10px rgba(34,197,94,0.4); }
        50% { transform: scale(1.05); box-shadow: 0 0 20px rgba(34,197,94,0.8); }
    }
</style>
