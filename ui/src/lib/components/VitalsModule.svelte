<script>
    export let vitals = { status: 'IDLE', cpu_usage: 0, ram_usage: '0/0', temperature: 0, resonance: 1.0 };
</script>

<div class="flex flex-col gap-1 font-mono text-[9px]">
    <div class="flex justify-between border-b border-cyan-900/20 pb-1 mb-1">
        <span class="text-zinc-600 uppercase tracking-widest">Telemetry</span>
        <span class="{vitals.status === 'CRITICAL' ? 'text-pink-600 animate-pulse' :
                      vitals.status === 'ONLINE'   ? 'text-emerald-500' : 'text-zinc-500'}">
            {vitals.status}
        </span>
    </div>
    <div class="flex justify-between">
        <span class="text-zinc-500">CPU</span>
        <span class="text-cyan-400">{vitals.cpu_usage?.toFixed(1) ?? '0.0'}%</span>
    </div>
    <div class="flex justify-between">
        <span class="text-zinc-500">RAM</span>
        <span class="text-cyan-400">{vitals.ram_usage}</span>
    </div>
    <div class="flex justify-between">
        <span class="text-zinc-500">HEAT</span>
        <span class="{vitals.temperature > 60 ? 'text-pink-500' : 'text-cyan-400'}">
            {vitals.temperature?.toFixed(1) ?? '0.0'}°C
        </span>
    </div>
    <div class="flex justify-between mt-1">
        <span class="text-zinc-500">RESONANCE</span>
        <span class="text-purple-400">{vitals.resonance?.toFixed(3) ?? '1.000'}</span>
    </div>
    <!-- Mini bars -->
    <div class="mt-1 space-y-1">
        <div class="h-0.5 bg-zinc-900 rounded overflow-hidden">
            <div class="h-full bg-cyan-500 transition-all duration-500" style="width:{vitals.cpu_usage ?? 0}%"></div>
        </div>
        <div class="h-0.5 bg-zinc-900 rounded overflow-hidden">
            <div class="h-full bg-purple-500 transition-all duration-500"
                 style="width:{(vitals.resonance ?? 1)*100}%"></div>
        </div>
    </div>
</div>
