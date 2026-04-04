<script>
    import { systemVitals } from '$lib/store';
</script>

<div class="flex gap-4 font-mono text-[9px] text-gray-500">
    <div class="flex gap-2">
        <span>CPU</span>
        <span class="text-cyan-400">{$systemVitals.cpu_usage.toFixed(1)}%</span>
    </div>
    <div class="flex gap-2">
        <span>RAM</span>
        <span class="text-purple-400">{$systemVitals.ram_usage}</span>
    </div>
    <div class="flex gap-2">
        <span>TEMP</span>
        <span class="{$systemVitals.temperature > 70 ? 'text-red-500' : 'text-orange-400'}">{$systemVitals.temperature.toFixed(1)}°C</span>
    </div>
</div>
