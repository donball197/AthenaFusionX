<script>
    import SoundFx from '$lib/SoundFx.js';
    export let response = "";
    export let isThinking = false;
    export let patchAvailable = false;
    export let onApplyPatch;
</script>

<div class="flex flex-col gap-2 h-full">
    <div class="text-[10px] text-purple-400 border-b border-purple-900/40 flex justify-between uppercase tracking-widest font-bold pb-1 shrink-0">
        <span>Gemma // Neural Link</span>
        {#if isThinking}<span class="animate-pulse text-purple-500 text-[8px]">● PROCESSING</span>{/if}
    </div>

    <div class="flex-1 min-h-[220px] text-[12px] text-[#39FF14] overflow-y-auto border border-blue-900/20 p-3 bg-[#020202] whitespace-pre-wrap font-mono leading-relaxed shadow-[inset_0_0_15px_rgba(0,0,0,1)] custom-scrollbar">
        <span class="drop-shadow-[0_0_5px_rgba(57,255,20,0.5)]">
            {response || ">> UPLINK STABLE..."}
        </span>
    </div>

    {#if patchAvailable}
        <button on:click={() => { SoundFx.success(); onApplyPatch(); }} 
                class="w-full bg-blue-600/20 border border-blue-500 text-blue-400 text-[9px] font-bold py-2 rounded animate-pulse mt-1 shrink-0">
            >> APPLY PATCH
        </button>
    {/if}
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e3a8a; }
</style>
