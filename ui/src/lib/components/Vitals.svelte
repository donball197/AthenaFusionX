<script>
    export let ram = "0MB";
    export let status = "OFFLINE";
    export let cpu = 0; // NEW: Passed from backend stats

    $: ramPercent = (parseInt(ram) || 0) / 2000 * 100;
</script>

<div class="bg-[#050505] border border-[#1a1a1a] p-4 font-mono text-[10px] flex flex-col gap-3">
    <div class="flex justify-between items-center border-b border-[#1a1a1a] pb-1">
        <span class="text-gray-500 uppercase tracking-tighter">System_Node</span>
        <span class={status === 'ONLINE' ? 'text-neon-green' : 'text-red-500'}>{status}</span>
    </div>

    <div class="space-y-1">
        <div class="flex justify-between text-gray-400">
            <span>CPU_LOAD</span>
            <span>{cpu.toFixed(1)}%</span>
        </div>
        <div class="h-1 w-full bg-[#111] rounded-full overflow-hidden">
            <div class="h-full bg-neon-green transition-all duration-300" style="width: {cpu}%"></div>
        </div>
    </div>

    <div class="space-y-1">
        <div class="flex justify-between text-gray-400">
            <span>RAM_INDEX</span>
            <span>{ram}</span>
        </div>
        <div class="h-1 w-full bg-[#111] rounded-full overflow-hidden">
            <div class="h-full bg-blue-500 transition-all duration-300 shadow-[0_0_5px_rgba(59,130,246,0.5)]" style="width: {ramPercent}%"></div>
        </div>
    </div>
</div>

<style>
    .text-neon-green { color: #39FF14; }
    .bg-neon-green { background-color: #39FF14; box-shadow: 0 0 8px #39FF14; }
</style>
