<script>
    export let logs = "";
    export let active = false;
    export let onToggleFiles; 
    export let version = "v0.41.0";
</script>

{#if logs && active}
<div class="absolute bottom-12 right-6 left-6 h-24 bg-[#0a0a0a] border border-[#30363d] rounded p-3 pointer-events-none z-20 transition-all duration-500 border-l-neon-green border-l-2 shadow-xl">
    <div class="text-[8px] text-gray-500 border-b border-gray-800 pb-1 flex justify-between uppercase tracking-widest mb-2 font-bold">
        <div class="flex items-center gap-4">
            <div class="flex items-center gap-1.5">
                <span class="w-1.5 h-1.5 bg-neon-green rounded-full animate-pulse"></span>
                <span>Ghost Link // Stream</span>
            </div>
            <button on:click|stopPropagation={onToggleFiles} 
                    class="pointer-events-auto bg-green-950/40 border border-green-500/30 px-2 py-0.5 text-neon-green hover:bg-neon-green hover:text-black transition-all cursor-pointer uppercase font-bold tracking-tighter">
                [ Files ]
            </button>
        </div>
        <span>{version}</span>
    </div>
    <div class="text-[10px] text-green-400/80 font-mono whitespace-pre opacity-90 leading-tight overflow-hidden italic">
        {logs}
    </div>
</div>
{/if}

<style>
    .text-neon-green { color: #39FF14; }
    .border-l-neon-green { border-left-color: #39FF14; }
</style>
