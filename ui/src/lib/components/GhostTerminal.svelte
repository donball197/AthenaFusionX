<script>
    export let logs = [];
    export let isThinking = false;
</script>

<div class="h-full flex flex-col bg-black p-4 font-mono text-xs border-t border-[#30363d]">
    <div class="flex-1 overflow-y-auto space-y-1 custom-scrollbar">
        {#each logs as log}
            <div class="flex gap-3">
                <span class="text-zinc-600 shrink-0">[{log.timestamp}]</span>
                <span class={log.level === 'ERROR' ? 'text-red-500' : 'text-green-400/90'}>
                    {log.message}
                </span>
            </div>
        {/each}
        {#if isThinking}
            <div class="text-blue-400 animate-pulse">&gt;&gt; CONDUCTOR_ANALYZING_NEURAL_DNA...</div>
        {/if}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1f2937; }
</style>
