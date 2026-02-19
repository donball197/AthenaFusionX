<script>
    // Pure CSS Animation = Zero CPU Load
</script>

<div class="fixed inset-0 pointer-events-none z-[50] overflow-hidden flex items-center justify-center font-mono">
    
    <div class="relative w-[500px] h-[500px] opacity-20 animate-[spin_10s_linear_infinite]">
        <div class="absolute inset-0 border border-cyan-500/50 rounded-full border-dashed"></div>
        <div class="absolute inset-4 border border-cyan-400/30 rounded-full"></div>
        <div class="absolute top-0 left-1/2 -translate-x-1/2 w-0.5 h-4 bg-cyan-500"></div>
        <div class="absolute bottom-0 left-1/2 -translate-x-1/2 w-0.5 h-4 bg-cyan-500"></div>
        <div class="absolute left-0 top-1/2 -translate-y-1/2 w-4 h-0.5 bg-cyan-500"></div>
        <div class="absolute right-0 top-1/2 -translate-y-1/2 w-4 h-0.5 bg-cyan-500"></div>
    </div>

    <div class="absolute top-4 right-4 flex flex-col items-end gap-1">
        <div class="flex items-center gap-2">
            <div class="w-2 h-2 bg-green-500 rounded-full animate-pulse"></div>
            <span class="text-[10px] text-cyan-400 tracking-[0.2em]">SYSTEM // ONLINE</span>
        </div>
        <span class="text-[8px] text-cyan-600">SECURE CONNECTION ESTABLISHED</span>
    </div>

    <div class="absolute bottom-4 left-4 border-l-2 border-b-2 border-cyan-500/50 w-8 h-8"></div>
    <div class="absolute top-4 left-4 border-l-2 border-t-2 border-cyan-500/50 w-8 h-8"></div>
    <div class="absolute bottom-4 right-4 border-r-2 border-b-2 border-cyan-500/50 w-8 h-8"></div>
</div>
