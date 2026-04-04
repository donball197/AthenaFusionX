<script>
    export let logs = "";
    export let active = false;
    export let onToggleFiles;
    export let version = "v0.41.0";
</script>

{#if active}
<div class="absolute bottom-8 left-6 right-6 h-28 bg-[#0a0a0a] border border-[#30363d] border-l-neon-green rounded p-4 pointer-events-none z-50 shadow-2xl">
    <div class="flex justify-between text-[9px] text-gray-500 border-b border-gray-800 pb-2 mb-2 uppercase tracking-widest">
        <div class="flex items-center gap-4">
            <div class="flex items-center gap-2">
                <span class="w-2 h-2 bg-neon-green rounded-full animate-pulse"></span>
                <span>GHOST LINK // LIVE STREAM</span>
            </div>
            <button 
                on:click|stopPropagation={onToggleFiles}
                class="pointer-events-auto px-3 py-0.5 border border-green-600 text-neon-green hover:bg-neon-green hover:text-black text-[9px] font-bold transition-all">
                [ FILES ]
            </button>
        </div>
        <span>{version}</span>
    </div>
    <div class="text-green-400/80 text-[10px] font-light whitespace-pre overflow-hidden leading-tight">
        {logs}
    </div>
</div>
{/if}

<style>
    .text-neon-green { color: #39FF14; }
    .border-l-neon-green { border-left-color: #39FF14; }
</style>
