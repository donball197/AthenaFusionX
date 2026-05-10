<script>
    import { systemVitals, vmoTelemetry } from '$lib/core/store.svelte';
</script>
<div class="h-full flex flex-col p-6 border-r border-cyan-900/30 font-mono select-none">
    <h2 class="text-cyan-600 font-black mb-8 tracking-[0.2em] text-xs">SUBSTRATE MUSCLE</h2>
    
    <div class="space-y-6 text-[10px]">
        <div class="flex justify-between items-end border-b border-cyan-900/20 pb-2">
            <span class="text-cyan-800">CPU_LOAD</span>
            <span class="text-cyan-400 font-bold text-sm">{((systemVitals.cpu_load||0)*100).toFixed(0)}%</span>
        </div>
        
        <div class="flex justify-between items-end border-b border-cyan-900/20 pb-2">
            <span class="text-cyan-800">RAM_ALLOC</span>
            <span class="text-purple-400 font-bold text-sm">{(systemVitals.ram_gb||0).toFixed(2)} GB</span>
        </div>
        
        <div class="flex justify-between items-end border-b border-cyan-900/20 pb-2">
            <span class="text-cyan-800">THERMAL</span>
            <span class="font-bold text-sm {systemVitals.temperature > 60 ? 'text-red-500' : 'text-emerald-500'}">
                {(systemVitals.temperature||0).toFixed(1)}°C
            </span>
        </div>
        
        <div class="flex justify-between items-end border-b border-cyan-900/20 pb-2">
            <span class="text-cyan-800">VMO_VAULTS</span>
            <span class="text-amber-400 font-bold text-sm">{vmoTelemetry.active_vmos}</span>
        </div>
    </div>
</div>
