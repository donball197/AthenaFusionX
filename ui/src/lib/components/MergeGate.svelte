<script>
    import { createEventDispatcher } from 'svelte';
    const dispatch = createEventDispatcher();

    export let active = false;
    export let path = "";
</script>

{#if active}
<div class="fixed bottom-24 right-8 w-80 bg-[#050505] border border-yellow-500/50 shadow-[0_0_20px_rgba(234,179,8,0.2)] z-[100] p-4 font-mono">
    <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
            <div class="w-2 h-2 bg-yellow-500 animate-pulse"></div>
            <span class="text-[10px] text-yellow-500 font-bold uppercase tracking-widest">Patch Detected</span>
        </div>
        <span class="text-[8px] text-gray-500 uppercase">{path}</span>
    </div>
    
    <p class="text-[10px] text-gray-400 mb-4 leading-relaxed">
        Nexus has generated a high-integrity patch. System is waiting for manual authorization to merge.
    </p>

    <div class="flex gap-2">
        <button 
            on:click={() => dispatch('apply')}
            class="flex-1 bg-yellow-500/10 border border-yellow-500 text-yellow-500 text-[10px] py-1.5 font-bold hover:bg-yellow-500 hover:text-black transition-all">
            COMMIT CHANGE
        </button>
        <button 
            on:click={() => dispatch('reject')}
            class="px-3 bg-red-950/20 border border-red-900 text-red-700 text-[10px] hover:bg-red-900 hover:text-white">
            ✕
        </button>
    </div>
</div>
{/if}
