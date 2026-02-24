<script>
    import { onMount } from 'svelte';
    import Editor from '$lib/components/Editor.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import Vitals from '$lib/components/Vitals.svelte';
    import Heartbeat from '$lib/components/Heartbeat.svelte'; 
    import TacticalBlade from '$lib/components/TacticalBlade.svelte';
    import ResponseModule from '$lib/components/ResponseModule.svelte';
    import GhostCortex from "$lib/components/GhostCortex.svelte";
    import MergeGate from '$lib/components/MergeGate.svelte'; 
    import SwarmMap from '$lib/components/SwarmMap.svelte';
    import SoundFx from '$lib/SoundFx.js'; 
    import { openTabs, activeTab } from '$lib/store.js';

    let tabs = [];
    let activeTabType = 'editor';
    let userPrompt = "";
    let neuralResponse = ">> HIVE ONLINE";
    let isThinking = false;
    let terminalInstance;
    let bladeOpen = false; 
    let patchAvailable = false;
    let patchContent = "";
    let targetPatchFile = ""; 
    let visionActive = false;
    let voiceMuted = false; 
    let vimMode = false;
    let vitals = { status: "OFFLINE", cpu_usage: 0, ram_usage: "0.0", resonance: 0.0 }; 
    let ghostLogs = ""; 
    let buildOutput = "";

    openTabs.subscribe(t => tabs = t);

    $: currentCode = tabs.find(t => t.path === $activeTab)?.content || "// Use Ctrl+K to open a file";
    $: currentPath = $activeTab || "NO_FILE_LOADED";

    function toggleMute() { voiceMuted = !voiceMuted; SoundFx.click(); }
    function toggleVim() { vimMode = !vimMode; SoundFx.success(); }

    async function openFile(path) {
        if (!tabs.find(t => t.path === path)) {
            const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
            if (res.ok) {
                const data = await res.json();
                openTabs.update(ts => [...ts, { path, content: data.content, isDirty: false }]);
            }
        }
        activeTab.set(path);
        activeTabType = 'editor';
    }

    function closeTab(path) {
        openTabs.update(ts => ts.filter(t => t.path !== path));
        if ($activeTab === path) activeTab.set(tabs[tabs.length - 1]?.path || null);
    }

    function updateCode(newCode) {
        openTabs.update(ts => ts.map(t => t.path === $activeTab ? { ...t, content: newCode, isDirty: true } : t));
    }
    
    async function save() { 
        const tab = tabs.find(t => t.path === $activeTab);
        if (tab) {
            await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: tab.path, content: tab.content}) }); 
            openTabs.update(ts => ts.map(t => t.path === $activeTab ? { ...t, isDirty: false } : t));
            SoundFx.success();
        }
    }
    
    async function applyPatch() {
        if (!patchContent || !targetPatchFile) return;
        await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: targetPatchFile, content: patchContent}) });
        patchAvailable = false;
        SoundFx.success();
        openTabs.update(ts => ts.map(t => t.path === targetPatchFile ? { ...t, content: patchContent, isDirty: false } : t));
    }

    async function triggerBuild() {
        await save();
        activeTabType = 'hive';
        buildOutput = ">> COMPILING...";
        const res = await fetch('/api/devops/build', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ path: ".", content: "" }) });
        const result = await res.json();
        buildOutput = result.log;
        
        if (!result.success) {
            SoundFx.error();
            userPrompt = `FIX_ERROR in ${currentPath}: ${buildOutput}\nCODE:\n${currentCode}`;
            setTimeout(executeAction, 500); 
        } else {
            SoundFx.success();
        }
    }

    async function executeAction() {
        if (!userPrompt.trim() || isThinking) return;
        isThinking = true; SoundFx.processing();
        const p = userPrompt; userPrompt = "";
        try {
            const res = await fetch('/api/prompt', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ prompt: p, context: currentCode, use_vision: visionActive }) });
            const data = await res.json();
            neuralResponse = data.response;

            if (neuralResponse.includes("```")) {
                const match = neuralResponse.match(/```(?:rust|js|json)?\s*([\s\S]*?)\`\`\`/i);
                if (match) {
                    patchContent = match[1].trim();
                    const fileMatch = neuralResponse.match(/FILE:\s*([^\s\n]+)/);
                    targetPatchFile = fileMatch ? fileMatch[1].trim() : $activeTab;
                    patchAvailable = true; 
                }
            }
        } catch (e) { neuralResponse = ">> ERROR: " + e.message; } finally { isThinking = false; }
    }

    onMount(() => { 
        openFile("src/main.rs"); 
        const interval = setInterval(() => { 
            fetch('/api/vitals').then(r => r.ok ? r.json() : null).then(v => { if(v) vitals = v; }); 
        }, 3000); 
        return () => clearInterval(interval);
    });
</script>

<div class="flex flex-col h-screen w-screen bg-black text-green-500 font-mono overflow-hidden relative">
    <header class="h-10 shrink-0 bg-[#161b22] border-b border-[#30363d] flex items-center px-4 justify-between z-50">
        <div class="flex items-center gap-4">
            <span class="text-[#58a6ff] font-bold text-[10px] tracking-widest uppercase">Nexus // Full_Cortex</span>
            <button on:click={toggleMute} class="text-xs">{voiceMuted ? '🔇' : '🔊'}</button>
        </div>
        <div class="flex gap-2 ml-4">
            <button on:click={() => activeTabType = 'editor'} class="text-[9px] px-3 py-1 border {activeTabType === 'editor' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">CODE</button>
            <button on:click={() => activeTabType = 'hive'} class="text-[9px] px-3 py-1 border {activeTabType === 'hive' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">HIVE</button>
            <button on:click={() => activeTabType = 'terminal'} class="text-[9px] px-3 py-1 border {activeTabType === 'terminal' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">TERM</button>
        </div>
        <div class="flex-1 max-w-xl px-4">
            <input class="w-full bg-[#0d1117] border border-[#30363d] text-white px-3 py-1 text-[10px] outline-none focus:border-blue-500" placeholder="Ask Nexus..." bind:value={userPrompt} on:keydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); executeAction(); } }} />
        </div>
        <div class="flex gap-2">
            <button class="bg-blue-600 text-white font-bold px-3 py-1 text-[9px] rounded hover:bg-blue-500" on:click={() => triggerBuild()}>BUILD</button>
            <button class="bg-green-600 text-black font-bold px-3 py-1 text-[9px] rounded hover:bg-green-500" on:click={save}>SAVE</button>
        </div>
    </header>

    <div class="flex-1 flex relative overflow-hidden">
        <aside class="w-56 border-r border-[#30363d] bg-[#0d1117] p-4 flex flex-col hidden md:flex shrink-0">
            <Vitals ram={vitals.ram_usage} status={vitals.status} cpu={vitals.cpu_usage} resonance={vitals.resonance} />
            <Heartbeat resonance={vitals.resonance} />
            <div class="mt-4 flex-1 overflow-y-auto">
                <ResponseModule response={neuralResponse} {isThinking} {patchAvailable} onApplyPatch={applyPatch} />
            </div>
        </aside>

        <main class="flex-1 flex flex-col bg-black relative">
            {#if activeTabType === 'editor'}
                <div class="absolute inset-0 flex flex-col">
                    <div class="flex gap-1 overflow-x-auto bg-[#0d1117] px-2 py-1 border-b border-[#30363d]">
                        {#each tabs as tab}
                            <button on:click={() => activeTab.set(tab.path)} class="px-3 py-1 text-[9px] flex items-center gap-1 {tab.path === $activeTab ? 'bg-blue-900/50 text-white shadow-[0_-1px_0_#58a6ff_inset]' : 'text-gray-500'}">
                                {tab.path.split('/').pop()}
                                {#if tab.isDirty}<span class="text-yellow-500">*</span>{/if}
                                <span on:click|stopPropagation={() => closeTab(tab.path)} class="ml-1 hover:text-red-500">×</span>
                            </button>
                        {/each}
                    </div>
                    <Editor bind:value={currentCode} bind:path={currentPath} {vimMode} onChange={updateCode} />
                    <TacticalBlade bind:open={bladeOpen} onFileSelect={openFile} onToggleVim={toggleVim} />
                    <GhostCortex logs={ghostLogs} active={true} {isThinking} response={neuralResponse} onCommand={(cmd) => {userPrompt = cmd; setTimeout(executeAction, 50);}} onToggleFiles={() => bladeOpen = !bladeOpen} />
                </div>
            {/if}

            {#if activeTabType === 'hive'}
                <div class="absolute inset-0 p-6 bg-[#0d1117] flex flex-col gap-6 overflow-y-auto">
                    <SwarmMap resonance={vitals.resonance} />
                    <div class="bg-black border border-gray-800 p-2 text-[10px] font-mono text-gray-400 h-64 overflow-y-auto">
                        <pre>{buildOutput}</pre>
                    </div>
                </div>
            {/if}
            
            {#if activeTabType === 'terminal'}
                <div class="absolute inset-0 bg-black">
                    <Terminal bind:this={terminalInstance} />
                </div>
            {/if}
        </main>
    </div>

    {#if patchAvailable}
    <div class="fixed bottom-10 right-10 z-[200]">
        <MergeGate active={true} path={targetPatchFile} on:apply={applyPatch} on:reject={() => patchAvailable = false} />
    </div>
    {/if}
</div>
