<script>
    import { pushTermLine } from './NexusWS.js';
    let logs = $state([{ msg: "BIO-DOME INITIALIZED", type: "system" }]);
    let directive = $state("");

    async function executeShielded() {
        if (!directive.trim()) return;
        pushTermLine(`\x1b[35m>> [SHIELDED_REQ]: ${directive}\x1b[0m`);
        
        const res = await fetch('/api/drone', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ directive })
        });
        
        if (res.ok) {
            logs.push({ msg: `DEPLOYED: ${directive}`, type: "action" });
            directive = "";
        } else {
            pushTermLine("!! [SHIELD_FAULT]: Ignition failed.", "ansi-red");
        }
    }
</script>

<div class="p-4 font-mono text-[10px] h-full flex flex-col gap-4 bg-[#050505]">
    <div class="border border-red-900/30 bg-red-950/5 p-4 flex-1 overflow-y-auto custom-scrollbar">
        <h3 class="text-red-500 font-bold mb-2 tracking-widest uppercase">>> SHIELD_LOGS_STREAM</h3>
        {#each logs as log}
            <div class="text-gray-400 py-0.5 border-l border-red-900/20 pl-2">
                <span class="text-red-900">[{log.type}]</span> {log.msg}
            </div>
        {/each}
    </div>
    <div class="flex gap-2 bg-black p-1 border border-red-900/30">
        <input bind:value={directive} 
               class="flex-1 bg-transparent text-red-400 p-2 outline-none" 
               placeholder="Enter untrusted directive..." 
               onkeydown={(e) => e.key === 'Enter' && executeShielded()}/>
        <button onclick={executeShielded} 
                class="bg-red-900/40 hover:bg-red-600 text-white px-6 font-bold transition-all uppercase text-[9px]">
            Ignite Shield
        </button>
    </div>
</div>
