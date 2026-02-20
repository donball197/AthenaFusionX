<script>
    import { onMount, tick } from 'svelte';
    import Editor from '$lib/components/Editor.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import Vitals from '$lib/components/Vitals.svelte';
    import AgentMap from '$lib/components/AgentMap.svelte';
    import TacticalBlade from '$lib/components/TacticalBlade.svelte';
    import Voice from '$lib/components/Voice.svelte';
    import History from '$lib/components/History.svelte';
    import ResponseModule from '$lib/components/ResponseModule.svelte';
    import GhostCortex from "$lib/components/GhostCortex.svelte";
    import VisualCortex from '$lib/components/VisualCortex.svelte';
    import MergeGate from '$lib/components/MergeGate.svelte'; 
    import SoundFx from '$lib/SoundFx.js'; 

    let currentFile = "src/main.rs";
    let code = "// System Ready.";
    let activeTab = 'editor'; 
    let userPrompt = "";
    let neuralResponse = ">> COMMAND READY";
    let isThinking = false;
    let isListening = false;
    let terminalInstance;
    let bladeOpen = false;
    let ghostLogs = "";
    let memories = []; 
    
    let containers = [];
    let buildOutput = "";
    let buildStatus = "IDLE";
    
    let patchAvailable = false;
    let patchContent = "";
    let targetPatchFile = ""; 
    let vimEnabled = false;
    let visionActive = false;
    let vitals = { status: "ONLINE", cpu_usage: 0, ram_usage: "0.0" };

    async function fetchMemories() { try { const res = await fetch('/api/memories'); if(res.ok) memories = await res.json(); } catch(e){} }

    async function openFile(path) {
        currentFile = path;
        const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
        if (res.ok) { const data = await res.json(); code = data.content; activeTab = 'editor'; }
    }

    async function triggerBuild() {
        activeTab = 'infra';
        await save(); buildStatus = "BUILDING";
        buildOutput = ">> COMPILING...";
        
        const res = await fetch('/api/devops/build', {
            method: 'POST', headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({ path: ".", content: "" })
        });
        
        const result = await res.json();
        buildOutput = result.log;
        buildStatus = result.success ? "SUCCESS" : "FAIL";

        if (!result.success) {
            SoundFx.error();
            neuralResponse = ">> BUILD FAILED. ANALYZING ERROR...";
            userPrompt = `The build failed. Fix the code in ${currentFile} based on this error:\n\n${result.log}`;
            executeAction(); 
        } else {
            SoundFx.success();
            neuralResponse = ">> BUILD SUCCESSFUL.";
        }
    }

    async function refreshContainers() { const res = await fetch('/api/devops/containers'); if(res.ok) containers = await res.json(); }

    async function applyNexusPatch() {
        if (!patchContent) return;
        SoundFx.click();
        const res = await fetch('/api/fs/write', {
            method: 'POST', headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({path: targetPatchFile, content: patchContent})
        });
        if (res.ok) {
            patchAvailable = false; patchContent = ""; SoundFx.success();
            await openFile(targetPatchFile); triggerBuild(); 
        }
    }

    async function save() {
        SoundFx.click();
        await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: currentFile, content: code}) });
    }

    async function executeAction() {
        if (!userPrompt || isThinking) return;
        SoundFx.processing(); isThinking = true;
        const promptToSend = userPrompt; userPrompt = ""; await tick(); patchAvailable = false;
        
        try {
            const res = await fetch('/api/prompt', {
                method: 'POST', headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ prompt: promptToSend, context: code, terminal_context: buildOutput || (terminalInstance ? terminalInstance.getLogs() : ""), use_vision: visionActive })
            });
            const data = await res.json(); neuralResponse = data.response;
            
            const fileMatch = neuralResponse.match(/FILE:\s*([^\n]+)/i);
            targetPatchFile = fileMatch ? fileMatch[1].trim() : currentFile;

            const match = neuralResponse.match(/```(?:rust|js|json)?\s*([\s\S]*?)\`\`\`/i);
            if (match) { patchContent = match[1].trim(); patchAvailable = true; }
            SoundFx.success();
        } catch (e) { neuralResponse = ">> ERROR"; } finally { isThinking = false; }
    }

    onMount(() => {
      openFile(currentFile); fetchMemories(); refreshContainers();
      const i = setInterval(() => { fetch('/api/vitals').then(r => r.ok ? r.json() : null).then(v => { if(v) vitals = v; }); refreshContainers(); }, 3000); 
      return () => clearInterval(i);
    });
</script>

<div class="flex flex-col h-screen w-screen bg-black text-green-500 font-mono overflow-hidden">
    <header class="h-10 shrink-0 bg-[#161b22] border-b border-[#30363d] flex items-center px-4 justify-between gap-4 z-50">
        <span class="text-[#58a6ff] font-bold text-[10px] tracking-widest uppercase">Nexus // DevOps</span>
        <div class="flex gap-2">
            <button on:click={() => activeTab = 'editor'} class="text-[9px] px-3 py-1 border {activeTab === 'editor' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">CODE</button>
            <button on:click={() => activeTab = 'infra'} class="text-[9px] px-3 py-1 border {activeTab === 'infra' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">INFRA & BUILD</button>
            <button on:click={() => activeTab = 'terminal'} class="text-[9px] px-3 py-1 border {activeTab === 'terminal' ? 'border-green-500 text-white' : 'border-[#30363d] text-gray-500'}">TERM</button>
        </div>
        <div class="flex-1 max-w-xl relative flex items-center">
            <input class="w-full bg-[#0d1117] border border-[#30363d] text-white px-3 py-1 text-[10px] outline-none focus:border-green-500" placeholder="DevOps Command..." bind:value={userPrompt} on:keydown={(e) => e.key === 'Enter' && executeAction()} />
        </div>
        <div class="flex items-center gap-2">
            <button class="bg-blue-600 text-white font-bold px-3 py-1 text-[9px] rounded" on:click={triggerBuild}>BUILD</button>
            <button class="bg-green-600 text-black font-bold px-3 py-1 text-[9px] rounded" on:click={save}>SAVE</button>
        </div>
    </header>

    <div class="flex-1 flex relative overflow-hidden">
        <aside class="w-64 border-r border-[#30363d] bg-[#0d1117] p-4 flex flex-col gap-6 hidden md:flex overflow-y-auto shrink-0 z-40 custom-scrollbar">
            <Vitals ram={vitals.ram_usage} status={vitals.status} cpu={vitals.cpu_usage} />
            <ResponseModule response={neuralResponse} {isThinking} {patchAvailable} onApplyPatch={applyNexusPatch} />
            <History {memories} onReplay={(m) => { code = m.solution_code; activeTab = 'editor'; }} />
        </aside>

        <main class="flex-1 flex flex-col bg-black relative">
            <section class="flex-1 relative overflow-hidden">
                <div class={activeTab === 'terminal' ? 'h-full' : 'hidden'}><Terminal bind:this={terminalInstance} /></div>
                
                <div class={activeTab === 'editor' ? 'h-full w-full relative' : 'hidden'}>
                    <Editor bind:value={code} path={currentFile} onChange={(v) => code = v} vimMode={vimEnabled} />
                    <GhostCortex logs={ghostLogs} response={neuralResponse} isThinking={isThinking} active={activeTab === "editor"} onCommand={(cmd) => { userPrompt = cmd; executeAction(); }} onToggleFiles={() => bladeOpen = !bladeOpen} />
                    <TacticalBlade bind:open={bladeOpen} onFileSelect={openFile} onToggleVim={() => vimEnabled = !vimEnabled} />
                </div>

                <div class={activeTab === 'infra' ? 'h-full w-full p-6 overflow-y-auto' : 'hidden'}>
                    <div class="grid grid-cols-2 gap-6">
                        <div>
                            <h2 class="text-white text-xs font-bold mb-4 uppercase border-b border-gray-800 pb-2">Build Output</h2>
                            <pre class="bg-[#0d1117] p-3 text-[10px] text-gray-400 font-mono h-96 overflow-y-auto border border-[#30363d] whitespace-pre-wrap">{buildOutput}</pre>
                        </div>
                        <div>
                            <h2 class="text-white text-xs font-bold mb-4 uppercase border-b border-gray-800 pb-2">Active Containers</h2>
                            <div class="flex flex-col gap-2">
                                {#each containers as c}
                                    <div class="bg-[#0d1117] p-2 border border-[#30363d] rounded flex justify-between items-center">
                                        <div><div class="text-green-400 text-[10px] font-bold">{c.names}</div><div class="text-gray-500 text-[9px]">{c.image}</div></div>
                                        <div class="text-[9px] uppercase {c.status.includes('Up') ? 'text-green-500' : 'text-red-500'}">{c.status}</div>
                                    </div>
                                {/each}
                            </div>
                        </div>
                    </div>
                </div>
            </section>
        </main>
    </div>
    <MergeGate active={patchAvailable} path={targetPatchFile} on:apply={applyNexusPatch} on:reject={() => { patchAvailable = false; patchContent = ""; }} />
</div>
