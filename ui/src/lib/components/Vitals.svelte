<script>
    export let ram = "0MB";
    export let status = "OFFLINE";
    $: ramPercent = (parseInt(ram) / 2000) * 100;
</script>

<div class="bg-[#050505] border border-[#1a1a1a] p-4 font-mono text-xs">
    <div class="flex justify-between mb-2">
        <span class="text-gray-500">SYSTEM_STATUS</span>
        <span class={status === 'ONLINE' ? 'text-neon-green' : 'text-red-500'}>{status}</span>
    </div>
    <div class="mb-1 text-gray-400">RAM_USAGE: {ram}</div>
    <div class="h-1 w-full bg-[#111]">
        <div class="h-full bg-neon-green transition-all duration-500" style="width: {ramPercent}%"></div>
    </div>
</div>
