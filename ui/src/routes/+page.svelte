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
  let code = "// Interface Stable.";
  let activeTab = 'editor';
  let userPrompt = "";
  let neuralResponse = ">> COMMAND READY";
  let isThinking = false;
  let isListening = false;
  let terminalInstance;
  let bladeOpen = false;
  let ghostLogs = "";
  let memories = []; 
  let patchAvailable = false;
  let patchContent = "";
  let vimEnabled = false;
  let visionActive = false;
  let sparkActive = false;
  let visionDebug = false;
  let debugFrame = "";

  let vitals = { status: "ONLINE", cpu_usage: 0, ram_usage: "0.1GB / 2.7GB" };
  let agents = [{id: 1, target: "main.rs"}, {id: 2, target: "IDLE"}];

  async function fetchMemories() {
      const res = await fetch('/api/memories');
      if (res.ok) memories = await res.json();
  }

  async function fetchDebugFrame() {
      if (!visionActive || !visionDebug) return;
      try {
          const res = await fetch('/api/vision/frame');
          const data = await res.json();
          if (data.status === 'ok' && data.response) {
              debugFrame = `data:image/jpeg;base64,${data.response}`;
          }
      } catch (e) { console.error("Optic Nerve Error:", e); }
  }

  async function openFile(path) {
      SoundFx.click();
      currentFile = path;
      const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
      if (res.ok) {
          const data = await res.json();
          code = data.content;
          activeTab = 'editor';
          bladeOpen = false;
      }
  }

  function triggerSpark() {
      sparkActive = true;
      setTimeout(() => sparkActive = false, 800);
  }

  // INDUSTRIAL SAFETY: The only way to apply AI code is through this gate
  async function applyNexusPatch() {
      if (!patchContent) return;
      code = patchContent;
      patchAvailable = false;
      await save(); 
      patchContent = "";
  }

  async function save() {
      SoundFx.click();
      const res = await fetch('/api/fs/write', {
          method: 'POST',
          headers: {'Content-Type': 'application/json'},
          body: JSON.stringify({path: currentFile, content: code})
      });
      const status = await res.json();
      if (res.ok) {
          triggerSpark();
          SoundFx.success();
      } else {
          neuralResponse = ">> SHIELD TRIGGERED: REVERTED";
          SoundFx.error();
      }
  }

  async function executeAction() {
      if (!userPrompt || isThinking) return;
      SoundFx.processing();
      isThinking = true;
      const promptToSend = userPrompt;
      userPrompt = ""; 
      await tick(); 
      patchAvailable = false;
      try {
          const res = await fetch('/api/prompt', {
              method: 'POST',
              headers: {'Content-Type': 'application/json'},
              body: JSON.stringify({ 
                  prompt: promptToSend, 
                  context: code, 
                  terminal_context: terminalInstance ? terminalInstance.getLogs() : "",
                  use_vision: visionActive
              })
          });
          const data = await res.json();
          neuralResponse = data.response;
          
          // Pattern Matcher for Code Blocks
          const match = neuralResponse.match(/```(?:rust|rs)?\s*([\s\S]*?)\`\`\`/i);
          if (match) { 
              patchContent = match[1].trim(); 
              patchAvailable = true; 
          }
          
          SoundFx.success();
          fetchMemories();
      } catch (e) { neuralResponse = ">> ERROR"; } finally { isThinking = false; }
  }

  function syncGhost() {
      if (terminalInstance && activeTab === 'editor') {
          const rawLogs = terminalInstance.getLogs().split('\n');
          ghostLogs = rawLogs.filter(line => line.trim() !== "").slice(-4).join('\n');
      }
  }

  onMount(() => {
    openFile(currentFile); fetchMemories();
    const i = setInterval(() => { 
        fetch('/api/vitals').then(r => r.ok ? r.json() : null).then(v => { if(v) vitals = v; });
        syncGhost();
        if (visionDebug) fetchDebugFrame();
    }, 1000);
    return () => clearInterval(i);
  });
</script>

<div class="flex flex-col h-screen w-screen bg-black text-green-500 font-mono overflow-hidden">
    {#if sparkActive}
        <div class="absolute inset-0 pointer-events-none z-[9999] bg-green-500/5 animate-pulse border-x-4 border-green-500/20"></div>
    {/if}

    <header class="h-10 shrink-0 bg-[#161b22] border-b border-[#30363d] flex items-center px-4 justify-between gap-4">
        <div class="flex items-center gap-4">
            <span class="text-[#58a6ff] font-bold text-[10px] tracking-widest uppercase">Nexus // Visor</span>
            <div class="flex gap-1">
                <button on:click={() => activeTab = 'editor'} class="text-[9px] px-3 py-1 border {activeTab === 'editor' ? 'border-[#58a6ff] bg-[#1f242c] text-white' : 'border-[#30363d] text-gray-500'}">EDITOR</button>
                <button on:click={() => activeTab = 'terminal'} class="text-[9px] px-3 py-1 border {activeTab === 'terminal' ? 'border-[#58a6ff] bg-[#1f242c] text-white' : 'border-[#30363d] text-gray-500'}">TERMINAL</button>
            </div>
        </div>

        <div class="flex-1 max-w-xl relative flex items-center">
            <Voice bind:userPrompt bind:isListening />
            <div class="absolute inset-0 bg-purple-900/10 border border-purple-900/30 rounded"></div>
            {#if isThinking}<div class="absolute inset-0 bg-purple-500/20 animate-pulse rounded"></div>{/if}
            <input class="w-full bg-transparent text-purple-200 pl-10 pr-4 py-0.5 text-[10px] outline-none placeholder-purple-900 relative z-10" 
                   placeholder={isListening ? "Listening..." : "Neural Uplink Command..."} 
                   bind:value={userPrompt} on:keydown={(e) => e.key === 'Enter' && executeAction()} />
        </div>

        <div class="flex items-center gap-4">
            <div class="flex items-center gap-1 border border-[#30363d] rounded p-0.5 bg-black/50">
                <button class="px-2 py-0.5 text-[10px] font-bold transition-colors {visionActive ? 'text-red-500' : 'text-gray-500'}" 
                        on:click={() => { visionActive = !visionActive; if(!visionActive) visionDebug = false; }}>
                    VISION: {visionActive ? 'ON' : 'OFF'}
                </button>
                {#if visionActive}
                    <button class="px-2 py-0.5 text-[10px] border-l border-[#30363d] {visionDebug ? 'text-yellow-400' : 'text-gray-600'}" 
                            on:click={() => { visionDebug = !visionDebug; if(visionDebug) fetchDebugFrame(); }}>
                        DEBUG
                    </button>
                {/if}
            </div>
            <button class="bg-neon-green text-black font-bold px-4 py-0.5 text-[10px] rounded" on:click={save}>SAVE</button>
        </div>
    </header>

    <div class="flex-1 flex relative overflow-hidden">
        {#if visionDebug && debugFrame}
            <div class="absolute bottom-10 right-4 w-48 border border-yellow-500/50 bg-black/90 p-1 z-[50] shadow-2xl">
                <div class="text-[8px] text-yellow-500 mb-1 flex justify-between">
                    <span>LIVE_OPTIC_FEED</span>
                    <button on:click={() => visionDebug = false}>×</button>
                </div>
                <img src={debugFrame} alt="Vision Feed" class="w-full grayscale brightness-125 contrast-125 border border-white/10" />
            </div>
        {/if}

        <aside class="w-64 border-r border-[#30363d] bg-[#0d1117] p-4 flex flex-col gap-6 hidden md:flex overflow-y-auto shrink-0 z-40">
            <Vitals ram={vitals.ram_usage} status={vitals.status} cpu={vitals.cpu_usage} />
            <ResponseModule response={neuralResponse} {isThinking} {patchAvailable} onApplyPatch={applyNexusPatch} />
            <History {memories} onRefresh={fetchMemories} onReplay={(m) => { code = m.solution_code; activeTab = 'editor'; }} />
            <AgentMap swarm={agents} />
        </aside>

        <main class="flex-1 flex flex-col bg-black relative">
            <section class="flex-1 relative overflow-hidden">
                <div class={activeTab === 'terminal' ? 'h-full' : 'hidden'}><Terminal bind:this={terminalInstance} /></div>
                <div class={activeTab === 'editor' ? 'h-full w-full relative' : 'hidden'}>
                    <Editor bind:value={code} path={currentFile} onChange={(v) => code = v} vimMode={vimEnabled} />
                    <VisualCortex active={visionActive} />
                    <GhostCortex logs={ghostLogs} response={neuralResponse} isThinking={isThinking} 
                                 active={activeTab === "editor"} 
                                 onCommand={(cmd) => { userPrompt = cmd; executeAction(); }} 
                                 onToggleFiles={() => bladeOpen = !bladeOpen} />
                    <TacticalBlade bind:open={bladeOpen} onFileSelect={openFile} onToggleVim={() => vimEnabled = !vimEnabled} />
                </div>
            </section>
        </main>
    </div>

    <MergeGate 
        active={patchAvailable} 
        path={currentFile} 
        on:apply={applyNexusPatch} 
        on:reject={() => { patchAvailable = false; patchContent = ""; }} />
</div>
