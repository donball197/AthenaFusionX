<script>
    export let active = false;
    export let events = [
        { time: "02:14", event: "NEURAL_CORE_REBOOT", status: "COMPLETE" },
        { time: "02:17", event: "MEMORY_FRAGMENT_RECOVERED", status: "SUCCESS" },
        { time: "02:21", event: "SWARM_RESYNC", status: "IN_PROGRESS" }
    ];

    $: displayedEvents = active ? events : [];
</script>

{#if active}
<div class="bg-black/95 border border-purple-500/40 p-5 font-mono text-xs shadow-2xl w-96">
    <div class="uppercase tracking-widest text-purple-400 text-[10px] border-b border-purple-900/50 pb-2 mb-4">
        LAZARUS TIMELINE // RESURRECTION_PROTOCOL
    </div>
    
    <div class="space-y-4">
        {#each displayedEvents as ev}
            <div class="flex gap-4">
                <div class="text-purple-500/70 w-12 shrink-0">{ev.time}</div>
                <div class="flex-1">
                    <div class="text-gray-200">{ev.event}</div>
                    <div class="text-[10px] {ev.status === 'COMPLETE' || ev.status === 'SUCCESS' ? 'text-green-400' : 'text-yellow-400'}">
                        {ev.status}
                    </div>
                </div>
            </div>
        {/each}
    </div>
    
    <div class="mt-6 text-[9px] text-gray-500 italic">
        Last resurrection cycle: 47 seconds ago
    </div>
</div>
{/if}
