<script>
    import { onMount } from 'svelte';
    import { WS } from './components/NexusWS.js';
    import { 
        pendingPatch, isGhostTyping, ghostBuffer, 
        terminalLogs, systemVitals, activeTab, layout 
    } from '$lib/store';

    import Editor         from './components/Editor.svelte';
    import GhostCortex    from './components/GhostCortex.svelte';
    import GhostTerminal  from './components/GhostTerminal.svelte';
    import Heartbeat      from './components/Heartbeat.svelte';
    import ResponseModule from './components/ResponseModule.svelte';
    import Vitals         from './components/Vitals.svelte';
    import VisualCortex   from './components/VisualCortex.svelte';
    import Explorer       from './components/Explorer.svelte';
    import Voice          from './components/Voice.svelte';

    let userPrompt = "";
    let neuralResponse = ">> SYSTEM_RES_NOMINAL";

    async function executeAction() {
        if (!userPrompt.trim() || $isGhostTyping) return;
        // Global WS singleton send
        WS.send({ type: 'directive', payload: userPrompt });
        userPrompt = "";
    }

    async function handleApplyPatch() {
        if (!$pendingPatch) return;
        
        try {
            const res = await fetch('/api/fs/write', { 
                method: 'POST', 
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ path: $pendingPatch.path, content: $pendingPatch.content })
            });
            if (!res.ok) throw new Error("Write failed");
            
            WS.send({ type: 'terminal', payload: `[NEXUS] Patch applied successfully to ${$pendingPatch.path}` });
            pendingPatch.set(null);
        } catch (e) {
            WS.send({ type: 'terminal', payload: `!! [FAULT] Failed to apply patch: ${e.message}` });
        }
    }
</script>

<div class="h-screen w-screen bg-black text-neon-green font-mono grid grid-cols-12 grid-rows-12 gap-1 p-1 overflow-hidden">
    <VisualCortex active={$layout.visionActive} />
    
    <header class="col-span-12 row-span-1 bg-[#0a0a0a] border-b border-[#30363d] flex items-center px-4 justify-between">
        <span class="font-bold text-neon-green tracking-widest">NEXUS SINGULARITY v1.9.1</span>
        <input 
            bind:value={userPrompt} 
            on:keydown={(e) => e.key === 'Enter' && executeAction()} 
            class="bg-black border border-green-900 w-1/2 px-2 py-1 text-xs outline-none focus:border-neon-green" 
            placeholder="INPUT DIRECTIVE..." 
        />
        <Voice bind:userPrompt isListening={$isGhostTyping} />
    </header>

    <div class="col-span-3 row-span-2">
        <Vitals 
            ram={$systemVitals.ram_usage} 
            cpu={$systemVitals.cpu_usage} 
            status={$systemVitals.status} 
            resonance={$systemVitals.resonance} 
        />
    </div>

    <div class="col-span-6 row-span-2 border border-blue-900/20">
        <!-- FIXED: Wired to global $ghostBuffer -->
        <GhostCortex response={$ghostBuffer} active={$isGhostTyping} />
    </div>

    <div class="col-span-3 row-span-2">
        <Heartbeat resonance={$systemVitals.resonance} />
    </div>

    <div class="col-span-2 row-span-9 border border-white/5">
        <Explorer open={$layout.sidebarVisible} onFileSelect={(p) => activeTab.set(p)} />
    </div>

    <div class="col-span-7 row-span-6">
        <Editor path={$activeTab} />
    </div>

    <div class="col-span-3 row-span-6 border border-white/5">
        <ResponseModule 
            response={$ghostBuffer} 
            isThinking={$isGhostTyping} 
            patchAvailable={$pendingPatch !== null} 
            onApplyPatch={handleApplyPatch} 
        />
    </div>

    <div class="col-span-10 row-span-3">
        <!-- FIXED: Wired to global $terminalLogs -->
        <GhostTerminal logs={$terminalLogs} isThinking={$isGhostTyping} />
    </div>
</div>

<style>
    :global(body) { margin: 0; background: black; overflow: hidden; }
</style>
