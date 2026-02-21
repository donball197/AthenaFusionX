<script>
    import { onMount, tick } from 'svelte';
    import Editor from '$lib/components/Editor.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import Vitals from '$lib/components/Vitals.svelte';
    import Heartbeat from '$lib/components/Heartbeat.svelte'; 
    import TacticalBlade from '$lib/components/TacticalBlade.svelte';
    import Voice from '$lib/components/Voice.svelte';
    import History from '$lib/components/History.svelte';
    import ResponseModule from '$lib/components/ResponseModule.svelte';
    import GhostCortex from "$lib/components/GhostCortex.svelte";
    import MergeGate from '$lib/components/MergeGate.svelte'; 
    import SoundFx from '$lib/SoundFx.js'; 

    let currentFile = "src/main.rs";
    let code = "// Hive Ready.";
    let activeTab = 'editor'; 
    let userPrompt = "";
    let neuralResponse = ">> HIVE ONLINE";
    let isThinking = false;
    let isAnalyzing = false;
    let dissectionProgress = 0;
    let isListening = false;
    let terminalInstance;
    let bladeOpen = false; 
    let memories = []; 
    let drones = []; 
    let buildOutput = "";
    let patchAvailable = false;
    let patchContent = "";
    let targetPatchFile = ""; 
    let visionActive = false;
    let voiceMuted = false; 
    let vitals = { status: "OFFLINE", cpu_usage: 0, ram_usage: "0.0", resonance: 0.0 }; 
    let ghostLogs = ""; 

    function toggleMute() { 
        voiceMuted = !voiceMuted; 
        if (voiceMuted) window.speechSynthesis.cancel();
        SoundFx.click(); 
    }

    function speak(text) {
        if (!voiceMuted && 'speechSynthesis' in window) {
            window.speechSynthesis.cancel();
            const utterance = new SpeechSynthesisUtterance(text);
            window.speechSynthesis.speak(utterance);
        }
    }

    async function fetchMemories() { try { const res = await fetch('/api/memories'); if(res.ok) memories = await res.json(); } catch(e){} }
    
    async function openFile(path) { 
        currentFile = path; 
        const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`); 
        if (res.ok) { 
            const data = await res.json(); 
            code = data.content; 
            activeTab = 'editor'; 
        } 
    }

    async function spawnDrone(type) { 
        SoundFx.click(); 
        await fetch('/api/hive/spawn', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({drone: type, action: "spawn"}) }); 
        refreshHive(); 
    }

    async function killAll() { 
        SoundFx.error(); 
        await fetch('/api/hive/kill_all', { method: 'POST' }); 
        refreshHive(); 
    }

    async function runCleanup() {
        SoundFx.processing();
        const res = await fetch('/api/system/cleanup', { method: 'POST' });
        const msg = await res.json();
        alert("Janitor: " + msg);
    }

    async function refreshHive() { 
        const res = await fetch('/api/hive/status'); 
        if(res.ok) drones = await res.json(); 
    }

    async function save() { 
        SoundFx.click(); 
        await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: currentFile, content: code}) }); 
    }

    async function applyPatch() {
        SoundFx.success();
        await fetch('/api/fs/write', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({path: targetPatchFile, content: patchContent}) });
        patchAvailable = false;
        speak("Patch applied.");
        if (targetPatchFile === currentFile) openFile(currentFile);
    }

    async function triggerBuild() {
        await save();
        buildOutput = ">> COMPILING...";
        activeTab = 'hive'; 
        const res = await fetch('/api/devops/build', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ path: ".", content: "" }) });
        const result = await res.json();
        buildOutput = result.log;
        
        if (!result.success) { 
            SoundFx.error(); 
            userPrompt = "The build failed. Fix this error in " + currentFile + ":\n" + buildOutput;
            executeAction(); 
        } else { 
            SoundFx.success(); 
            speak("Build Successful."); 
        }
    }

    async function executeAction() {
        if (!userPrompt || isThinking) return;
        const isDissecting = userPrompt.toLowerCase().startsWith("dissect ");
        if (isDissecting) { isAnalyzing = true; dissectionProgress = 0; }
        
        isThinking = true; SoundFx.processing();
        const promptToSend = userPrompt; userPrompt = ""; await tick();

        let poller;
        if (isDissecting) {
            poller = setInterval(async () => {
                const res = await fetch('/api/dissect/progress');
                if (res.ok) {
                    dissectionProgress = await res.json();
                    if (dissectionProgress >= 100) clearInterval(poller);
                }
            }, 500);
        }

        try {
            const res = await fetch('/api/prompt', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ prompt: promptToSend, context: code, terminal_context: buildOutput || (terminalInstance ? terminalInstance.getLogs() : ""), use_vision: visionActive }) });
            const data = await res.json(); 
            neuralResponse = data.response;
            
            if (neuralResponse.includes("```")) {
                speak("Patch generated.");
            } else {
                speak(neuralResponse);
            }

            const match = neuralResponse.match(/```(?:rust|js|json)?\s*([\s\S]*?)\`\`\`/i);
            if (match) { 
                patchContent = match[1].trim(); 
                patchAvailable = true; 
            }
            
            const fileMatch = neuralResponse.match(/FILE:\s*([^\n]+)/);
            targetPatchFile = fileMatch ? fileMatch[1].trim() : currentFile;

        } catch (e) { neuralResponse = ">> ERROR"; } finally { isThinking = false; isAnalyzing = false; if(poller) clearInterval(poller); }
    }

    function syncGhost() {
        if (terminalInstance) {
            try { } catch(e) {}
        }
    }

    onMount(() => { 
        openFile(currentFile); 
        fetchMemories(); 
        refreshHive(); 
        const interval = setInterval(() => { 
            fetch('/api/vitals').then(r => r.ok ? r.json() : null).then(v => { if(v) vitals = v; }); 
            refreshHive();
            syncGhost();
        }, 3000); 
        return () => clearInterval(interval);
    });
</script>

<div class="flex flex-col h-screen w-screen bg-black text-green-500 font-mono overflow-hidden">
    <header class="h-10 shrink-0 bg-[#161b22] border-b border-[#30363d] flex items-center px-4 justify-between z-50">
        <div class="flex items-center gap-4">
            <span class="text-[#58a6ff] font-bold text-[10px] tracking-widest uppercase">Nexus // Hive</span>
            <button on:click={toggleMute} class="text-xs hover:scale-110 transition-transform">
                {voiceMuted ? '🔇' : '🔊'}
            </button>
        </div>
        
        <div class="flex gap-2 ml-4">
            <button on:click={() => activeTab = 'editor'} class="text-[9px] px-3 py-1 border {activeTab === 'editor' ? 'border-green-500 text-white bg-green-900/20' : 'border-[#30363d] text-gray-500'}">CODE</button>
            <button on:click={() => activeTab = 'hive'} class="text-[9px] px-3 py-1 border {activeTab === 'hive' ? 'border-green-500 text-white bg-green-900/20' : 'border-[#30363d] text-gray-500'}">HIVE</button>
            <button on:click={() => activeTab = 'terminal'} class="text-[9px] px-3 py-1 border {activeTab === 'terminal' ? 'border-green-500 text-white bg-green-900/20' : 'border-[#30363d] text-gray-500'}">TERM</button>
        </div>

        <div class="flex-1 max-w-xl relative flex items-center px-4">
            <Voice bind:userPrompt={userPrompt} bind:isListening={isListening} disabled={voiceMuted} />
            <input class="w-full bg-[#0d1117] border border-[#30363d] text-white pl-8 pr-3 py-1 text-[10px] focus:border-green-500 outline-none" placeholder="Command..." bind:value={userPrompt} on:keydown={(e) => e.key === 'Enter' && executeAction()} />
        </div>
        
        <div class="flex gap-2">
            <button class="bg-red-900/40 text-red-500 font-bold px-3 py-1 text-[9px] rounded hover:bg-red-600 hover:text-white" on:click={runCleanup}>PURGE</button>
            <button class="bg-blue-600 text-white font-bold px-3 py-1 text-[9px] rounded hover:bg-blue-500" on:click={triggerBuild}>BUILD</button>
            <button class="bg-green-600 text-black font-bold px-3 py-1 text-[9px] rounded hover:bg-green-500" on:click={save}>SAVE</button>
        </div>
    </header>

    <div class="flex-1 flex relative overflow-hidden">
        <aside class="w-64 border-r border-[#30363d] bg-[#0d1117] p-4 flex flex-col hidden md:flex shrink-0 z-40 overflow-hidden">
            <div class="shrink-0 space-y-4 mb-4">
                <Vitals ram={vitals.ram_usage} status={vitals.status} cpu={vitals.cpu_usage} resonance={vitals.resonance} />
                <Heartbeat resonance={vitals.resonance} />
            </div>

            <div class="flex-1 min-h-0 flex flex-col border-t border-[#30363d] pt-4 overflow-hidden">
                <div class="flex-1 overflow-y-auto custom-scrollbar pb-10">
                    <ResponseModule response={neuralResponse} {isThinking} {patchAvailable} onApplyPatch={applyPatch} />
                </div>
            </div>
        </aside>

        <main class="flex-1 flex flex-col bg-black relative">
            {#if isAnalyzing}
                <div class="absolute inset-0 z-50 bg-[#0d1117]/95 flex flex-col items-center justify-center p-8 backdrop-blur-sm">
                    <div class="text-green-500 font-mono text-xl mb-4 animate-pulse">>> INITIATING SEC-OPS DISSECTION...</div>
                    <div class="w-full max-w-md border border-green-500/50 h-6 p-1">
                        <div class="bg-green-500 h-full transition-all duration-500" style="width: {dissectionProgress}%"></div>
                    </div>
                    <div class="text-green-400 mt-2 text-sm">{dissectionProgress}% COMPLETE</div>
                </div>
            {/if}

            {#if activeTab === 'editor'}
                <div class="absolute inset-0 flex flex-col w-full h-full">
                    <div class="flex-1 overflow-y-auto custom-scrollbar relative pb-48">
                        <Editor bind:value={code} path={currentFile} onChange={(v) => code = v} />
                    </div>
                    <TacticalBlade bind:open={bladeOpen} onFileSelect={openFile} />
                    <GhostCortex logs={ghostLogs} response={neuralResponse} isThinking={isThinking} active={true} onCommand={(cmd) => { userPrompt = cmd; executeAction(); }} onToggleFiles={() => bladeOpen = !bladeOpen} />
                </div>
            {/if}

            {#if activeTab === 'terminal'}
                <div class="absolute inset-0 w-full h-full bg-black">
                    <Terminal bind:this={terminalInstance} />
                </div>
            {/if}

            {#if activeTab === 'hive'}
                <div class="absolute inset-0 w-full h-full p-6 overflow-y-auto custom-scrollbar bg-[#0d1117]">
                    <div class="flex justify-between items-center mb-6">
                        <h1 class="text-white text-xl font-bold">Active Drones</h1>
                        <button class="bg-red-600 text-white px-4 py-2 text-xs font-bold rounded hover:bg-red-700" on:click={killAll}>⚠️ KILL SWITCH</button>
                    </div>
                    
                    <div class="grid grid-cols-3 gap-6 mb-8">
                        {#each ['secops', 'devops', 'industrial'] as type}
                        <div class="bg-[#0a0a0a] border border-[#30363d] p-4 rounded hover:border-green-500 transition-colors">
                            <span class="font-bold text-white block mb-2 uppercase text-xs">{type} Unit</span>
                            <button class="w-full bg-green-900/30 text-green-400 py-2 text-xs font-bold border border-green-900 hover:bg-green-500 hover:text-black" on:click={() => spawnDrone(type)}>SPAWN</button>
                        </div>
                        {/each}
                    </div>

                    <div class="mt-8 border-t border-[#30363d] pt-4">
                        <h2 class="text-white text-xs font-bold mb-4 uppercase">Build Output Log</h2>
                        <pre class="bg-black border border-[#30363d] p-4 text-[10px] text-gray-400 font-mono h-64 overflow-y-auto">{buildOutput}</pre>
                    </div>
                </div>
            {/if}
        </main>
    </div>
    
    <MergeGate 
        active={patchAvailable} 
        path={targetPatchFile} 
        on:apply={applyPatch} 
        on:reject={() => patchAvailable = false} 
    />
</div>
