<script>
    export let ram = "0.0";
    export let status = "OFFLINE";
    export let cpu = 0;
    export let resonance = 0.0;
</script>

<div class="vitals-container p-3 border border-[#30363d] bg-black text-[10px] uppercase tracking-tighter">
    <div class="flex justify-between items-center mb-1">
        <span class="text-gray-500">System Status</span>
        <span class={status === 'CRITICAL' ? 'text-red-500 animate-pulse' : 'neon-green'}>{status}</span>
    </div>
    <div class="mb-1">
        <div class="flex justify-between text-[8px] mb-0.5">
            <span>CPU LOAD</span>
            <span>{cpu.toFixed(1)}%</span>
        </div>
        <div class="w-full bg-gray-900 h-1">
            <div class="bg-blue-500 h-full" style="width: {cpu}%"></div>
        </div>
    </div>
    <div>
        <div class="flex justify-between text-[8px] mb-0.5">
            <span>MEM_UTIL</span>
            <span>{ram}</span>
        </div>
        <div class="flex gap-1 text-[7px] text-gray-600">
            <span>RES_FREQ: {(resonance * 528).toFixed(0)}Hz</span>
        </div>
    </div>
</div>

<style>
    .neon-green { color: #39FF14; }
    .vitals-container { box-shadow: 0 0 10px rgba(57, 255, 20, 0.05); }
</style>
