<script>
    import { ghostThinking } from '$lib/store';
    import { WS } from '$lib/components/NexusWS';

    let { logs = [], response = "", active = false } = $props();

    function acceptPatch() {
        WS.send({ type: "action", action: "accept_snapshot", data: logs.join("") });
    }
    function rejectPatch() {
        WS.send({ type: "action", action: "reject_patch" });
    }
    function saveAndBuild() {
        WS.send({ type: "action", action: "cargo_build" });
    }
</script>

<div class="flex flex-col h-full min-h-0 overflow-hidden bg-[#0d1117]">
    <div class="flex justify-between items-center padding-2 px-3 bg-[#161b22] border-b border-[#30363d]">
        <span class="text-[10px] font-bold text-[#8b949e] tracking-widest">GHOST_CORTEX</span>
        <span class="w-2 h-2 rounded-full {active ? 'bg-blue-500 animate-pulse shadow-[0_0_8px_#58a6ff]' : 'bg-[#30363d]'}"></span>
    </div>
    
    <div class="flex-1 p-2 overflow-y-auto text-[11px] text-[#c9d1d9] font-mono leading-tight whitespace-pre-wrap">
        {#if logs.length > 0}{logs.join("")}{:else if response}{response}{:else}>> AWAITING NEURAL STREAM...{/if}
    </div>

    {#if !$ghostThinking && (logs.length > 0 || response.length > 10)}
    <div class="flex gap-1 p-1 bg-[#161b22] border-t border-[#30363d]">
        <button class="flex-1 bg-[#0d1117] border border-[#30363d] text-[9px] text-[#8b949e] py-1.5 hover:text-green-400" onclick={acceptPatch}>[ ACCEPT ]</button>
        <button class="flex-1 bg-[#0d1117] border border-[#30363d] text-[9px] text-[#8b949e] py-1.5 hover:text-blue-400" onclick={saveAndBuild}>[ BUILD ]</button>
        <button class="flex-1 bg-[#0d1117] border border-[#30363d] text-[9px] text-[#8b949e] py-1.5 hover:text-red-400" onclick={rejectPatch}>[ REJECT ]</button>
    </div>
    {/if}
</div>
