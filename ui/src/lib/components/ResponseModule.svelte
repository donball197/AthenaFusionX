<script>
    import Response from './Response.svelte';
    export let response = "";
    export let isThinking = false;
    export let patchAvailable = false;
    export let onApplyPatch;
</script>

<div class="flex flex-col min-h-[200px] max-h-[400px] bg-[#050505] border border-blue-900/30 p-0 rounded shadow-2xl relative overflow-hidden group">
    <!-- Header -->
    <div class="bg-[#0a0a0a] border-b border-blue-900/20 px-3 py-1 flex justify-between items-center">
        <span class="text-[9px] font-bold text-blue-500 uppercase tracking-widest">Neural Link</span>
        {#if isThinking}
            <div class="w-2 h-2 rounded-full bg-blue-500 animate-ping"></div>
        {/if}
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-3">
        <Response {response} {isThinking} patchAvailable={false} />
    </div>

    <!-- ACTION GATE -->
    {#if patchAvailable}
        <div class="absolute bottom-0 left-0 right-0 p-2 bg-gradient-to-t from-black to-transparent">
            <button 
                on:click={onApplyPatch}
                class="w-full bg-green-600 hover:bg-green-500 text-black font-bold py-2 text-xs rounded uppercase tracking-widest shadow-[0_0_15px_rgba(34,197,94,0.4)] animate-pulse"
            >
                [ APPLY PATCH ]
            </button>
        </div>
    {/if}
</div>
