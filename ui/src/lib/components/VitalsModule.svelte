<script>
    import { systemVitals } from '$lib/core/store.svelte.js';
</script>

<div class="flex items-center gap-6 text-[10px] font-mono text-gray-400">
    <div class="flex items-center gap-1.5">
        <span>CPU</span>
        <span class="text-cyan-400 font-medium">{Math.round((systemVitals.cpu_load || 0) * 100)}%</span>
    </div>
    <div class="flex items-center gap-1.5">
        <span>RAM</span>
        <span class="text-purple-400">{(systemVitals.ram_gb || 0).toFixed(2)}GB</span>
    </div>
    <div class="flex items-center gap-1.5">
        <span>TEMP</span>
        <span class="{systemVitals.temperature > 65 ? 'text-red-400' : 'text-orange-400'}">
            {(systemVitals.temperature || 0).toFixed(1)}°C
        </span>
    </div>
    <div class="flex items-center gap-1.5">
        <span>RES</span>
        <span class="text-purple-400">{(systemVitals.resonance || 0).toFixed(2)}</span>
    </div>
</div>
