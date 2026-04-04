<script>
</script>

<div class="fixed inset-0 pointer-events-none z-0 overflow-hidden flex items-center justify-center opacity-10">
    <div class="relative w-[820px] h-[820px] animate-[spin_28s_linear_infinite]">
        <div class="absolute inset-0 border border-cyan-400/30 rounded-full border-dashed"></div>
        <div class="absolute inset-12 border border-cyan-500/20 rounded-full"></div>
        <div class="absolute inset-28 border border-cyan-600/10 rounded-full"></div>
    </div>
    
    <!-- Corner brackets -->
    <div class="absolute top-8 left-8 w-8 h-8 border-l border-t border-cyan-400/60"></div>
    <div class="absolute top-8 right-8 w-8 h-8 border-r border-t border-cyan-400/60"></div>
    <div class="absolute bottom-8 left-8 w-8 h-8 border-l border-b border-cyan-400/60"></div>
    <div class="absolute bottom-8 right-8 w-8 h-8 border-r border-b border-cyan-400/60"></div>
</div>

<style>
    @keyframes spin {
        from { transform: rotate(0deg); }
        to { transform: rotate(360deg); }
    }
</style>
