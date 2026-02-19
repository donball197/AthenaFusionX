<script>
    // System Frame: Background Layer
</script>

<div class="fixed inset-0 pointer-events-none z-0 overflow-hidden flex items-center justify-center font-mono opacity-10">
    <div class="relative w-[800px] h-[800px] animate-[spin_20s_linear_infinite]">
        <div class="absolute inset-0 border border-cyan-500/30 rounded-full border-dashed"></div>
        <div class="absolute inset-20 border border-cyan-400/10 rounded-full"></div>
    </div>

    <div class="absolute top-2 left-2 border-l border-t border-cyan-500/50 w-4 h-4"></div>
    <div class="absolute top-2 right-2 border-r border-t border-cyan-500/50 w-4 h-4"></div>
    <div class="absolute bottom-2 left-2 border-l border-b border-cyan-500/50 w-4 h-4"></div>
    <div class="absolute bottom-2 right-2 border-r border-b border-cyan-500/50 w-4 h-4"></div>
</div>

<style>
    @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
