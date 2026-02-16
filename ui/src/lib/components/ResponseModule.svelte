<script>
    import SoundFx from '$lib/SoundFx.js';
    export let response = "";
    export let isThinking = false;
    export let patchAvailable = false;
    export let onApplyPatch;
</script>

<div class="flex flex-col gap-2">
    <div class="text-[10px] text-purple-500 border-b border-purple-900 flex justify-between uppercase tracking-widest font-bold">
        <span>Neural Link</span>{#if isThinking}<span class="animate-pulse">●</span>{/if}
    </div>
    <div class="text-[9px] text-gray-400 h-32 overflow-y-auto border border-[#30363d] p-2 bg-black whitespace-pre-wrap leading-relaxed shadow-inner scrollbar-hide">
        {response}
    </div>
    {#if patchAvailable}
        <button on:click={() => { SoundFx.success(); onApplyPatch(); }} 
                class="w-full bg-purple-700 text-white text-[10px] font-bold py-1.5 rounded animate-pulse shadow-[0_0_15px_#9333ea] mt-1">
            >> APPLY NEURAL PATCH
        </button>
    {/if}
</div>

<style>
    .scrollbar-hide::-webkit-scrollbar { display: none; }
</style>
