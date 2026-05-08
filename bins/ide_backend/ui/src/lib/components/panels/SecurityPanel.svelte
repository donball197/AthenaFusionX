<script>
    import { TaskService } from '$lib/services/TaskService';
    import { onMount } from 'svelte';

    let vulnerabilityCount = $state(0);
    let lastAudit = $state('NEVER');
    let auditing = $state(false);

    async function runAudit() {
        auditing = true;
        // This triggers the Agent to perform a self-audit
        await TaskService.runDirective('run cargo audit and report only the total count of vulnerabilities', 'SECURITY');
        auditing = false;
        lastAudit = new Date().toLocaleTimeString();
    }

    onMount(() => {
        runAudit();
        const interval = setInterval(runAudit, 3600000); // Hourly heartbeat
        return () => clearInterval(interval);
    });
</script>

<div class="p-3 border border-red-900/30 bg-red-950/5 rounded font-mono text-[10px]">
    <div class="flex justify-between items-center mb-2">
        <span class="text-red-500 font-bold tracking-widest">SUBSTRATE_INTEGRITY</span>
        <button onclick={runAudit} class="text-gray-500 hover:text-red-400 transition-colors">
            {auditing ? '[SCANNING...]' : '[RE-AUDIT]'}
        </button>
    </div>
    <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full {vulnerabilityCount > 0 ? 'bg-red-500 animate-pulse' : 'bg-green-500'}"></div>
        <span class="text-gray-400">VULNERABILITIES: <span class="text-white">{vulnerabilityCount}</span></span>
    </div>
    <div class="text-gray-600 mt-1 uppercase">Last Check: {lastAudit}</div>
</div>
