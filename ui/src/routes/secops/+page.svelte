<script>
    import { runTask } from '$lib/api.js';
    import { pushTermLine } from '$lib/core/NexusWS.js';
    let isAuditing = $state(false);

    async function igniteAudit() {
        isAuditing = true;
        pushTermLine(">> [SECOPS] Initiating Bio-Dome Integrity Scan...", "text-cyan-400");
        const res = await runTask("Perform a comprehensive security audit of all .sh, .py, and .env files.");
        pushTermLine(res.output);
        isAuditing = false;
    }
</script>

<div class="p-8 h-full bg-[#050000] grid grid-cols-2 gap-8 font-mono">
    <div class="border border-red-900/40 p-6 bg-red-950/5 flex flex-col items-center justify-center gap-6">
        <h2 class="text-red-500 text-xl font-black italic tracking-tighter uppercase">Bio-Dome Shield</h2>
        <div class="w-48 h-48 rounded-full border-2 border-red-900 border-dashed flex items-center justify-center {isAuditing ? 'animate-spin' : ''}">
            <span class="text-red-600 font-bold text-4xl">SEC</span>
        </div>
        <button onclick={igniteAudit} class="w-full py-4 border border-red-600 text-red-500 hover:bg-red-600 hover:text-black font-black transition-all">
            [ TRIGGER_ISOLATION_CHECK ]
        </button>
    </div>
    <div class="border border-red-900/10 p-6 text-[10px] text-gray-600 space-y-4">
        <h3 class="text-red-900 font-bold uppercase underline">Threat_Intelligence</h3>
        <p>> PODMAN_SHIELD: ACTIVE</p>
        <p>> ZIRCON_IPC: NOMINAL</p>
        <p>> HOST_LEAK_PROTECTION: 100%</p>
        <div class="mt-8 pt-4 border-t border-red-900/20">
            <span class="text-red-500 font-bold">WARNING:</span> Any sandbox escape attempt will result in immediate kernel-level thread termination.
        </div>
    </div>
</div>
