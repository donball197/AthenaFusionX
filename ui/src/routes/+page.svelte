<script>
  import { onMount } from 'svelte';
  import Editor from '$lib/components/Editor.svelte';
  import Terminal from '$lib/components/Terminal.svelte';
  import Vitals from '$lib/components/Vitals.svelte';
  import AgentMap from '$lib/components/AgentMap.svelte';

  let currentFile = "src/main.rs";
  let code = "// Loading Neural Interface...";
  let isDirty = false;
  let activeTab = 'editor';
  
  let userPrompt = "";
  let neuralResponse = ">> WAITING FOR INPUT...";
  let neuralLatency = 0;
  let isThinking = false;
  let isListening = false; 
  let recognition;
  let terminalInstance;

  let vitals = { status: "OFFLINE", cpu_usage: 0, ram_usage: "0MB", integrity: 100 };
  let agents = [{id: 1, target: "main.rs"}, {id: 2, target: "IDLE"}];

  // --- FILE SYSTEM OPERATIONS ---
  async function load() {
      try {
        const res = await fetch(`/api/fs/read?path=${encodeURIComponent(currentFile)}`);
        if (res.ok) {
            const data = await res.json();
            code = data.content;
            isDirty = false;
        }
      } catch (e) { code = "// Link Error"; }
  }

  async function save() {
      await fetch('/api/fs/write', {
          method: 'POST',
          headers: {'Content-Type': 'application/json'},
          body: JSON.stringify({path: currentFile, content: code})
      });
      isDirty = false;
  }

  // --- TELEMETRY ---
  async function updateVitals() {
      try {
          const res = await fetch('/api/vitals');
          if (res.ok) vitals = await res.json();
          else vitals.status = "ERROR";
      } catch (e) { vitals.status = "OFFLINE"; }
  }

  // --- VOICE ---
  function toggleVoice() {
      if (!recognition) return;
      if (isListening) {
          recognition.stop();
          isListening = false;
      } else {
          userPrompt = ""; 
          try { recognition.start(); isListening = true; } catch (e) { console.error(e); }
      }
  }

  // --- NEURAL LINK (FIXED + VISUALS) ---
  async function sendNeuralPrompt() {
      if (!userPrompt || isThinking) return;
      
      // FIX 1: Capture and clear immediately
      const capturedPrompt = userPrompt;
      userPrompt = ""; 
      
      isThinking = true;
      neuralResponse = ">> UPLOADING CONTEXT TO GEMMA-3...";
      
      try {
          const res = await fetch('/api/prompt', {
              method: 'POST',
              headers: {'Content-Type': 'application/json'},
              body: JSON.stringify({ prompt: capturedPrompt, context: code })
          });
          
          if (res.ok) {
              const data = await res.json();
              neuralResponse = data.response;
              neuralLatency = data.latency; 

              // AGENT HANDS
              const match = neuralResponse.match(/\[EXECUTE:\s*(.*?)\]/);
              if (match && match[1]) {
                  const command = match[1];
                  activeTab = 'terminal';
                  if (terminalInstance) {
                      setTimeout(() => terminalInstance.triggerCommand(command), 500);
                  }
              }
          } else {
              neuralResponse = ">> NEURAL_LINK_FAILURE";
          }
      } catch (e) {
          neuralResponse = ">> CRITICAL_ERROR: " + e.message;
      } finally {
          isThinking = false;
      }
  }

  onMount(() => {
    load();
    updateVitals();
    const i = setInterval(updateVitals, 2000);

    if ('webkitSpeechRecognition' in window) {
        // @ts-ignore
        recognition = new webkitSpeechRecognition();
        recognition.continuous = false;
        recognition.interimResults = true;
        recognition.onstart = () => { isListening = true; };
        recognition.onend = () => { isListening = false; };
        recognition.onresult = (event) => { userPrompt = event.results[0][0].transcript; };
    }
    return () => clearInterval(i);
  });
</script>

<div class="flex flex-col h-screen w-screen overflow-hidden bg-black text-green-500 font-mono">
    <header class="h-10 shrink-0 bg-[#161b22] border-b border-[#30363d] flex items-center px-4 justify-between gap-4">
        <div class="flex items-center gap-6">
            <span class="text-[#58a6ff] font-bold text-xs tracking-widest hidden md:block">NEXUS // VISOR</span>
            <div class="flex gap-1">
                <button on:click={() => activeTab = 'editor'} class="text-[9px] px-3 py-1 border transition-colors {activeTab === 'editor' ? 'border-[#58a6ff] bg-[#1f242c] text-white' : 'border-[#30363d] text-gray-500'}">EDITOR</button>
                <button on:click={() => activeTab = 'terminal'} class="text-[9px] px-3 py-1 border transition-colors {activeTab === 'terminal' ? 'border-[#58a6ff] bg-[#1f242c] text-white' : 'border-[#30363d] text-gray-500'}">TERMINAL</button>
            </div>
        </div>

        <div class="flex-1 max-w-xl flex items-center gap-2 relative">
            <button on:click={toggleVoice} 
                    class="z-10 text-[9px] font-bold px-2 py-0.5 rounded transition-colors w-24 flex justify-center items-center gap-2 
                    {isListening ? 'bg-red-600 text-white animate-pulse' : 'bg-[#1f242c] text-gray-500 border border-gray-700 hover:border-gray-500'}">
                {#if isListening}<span>● LIVE</span>{:else}<span>🔇 MUTED</span>{/if}
            </button>
            
            <div class="flex-1 relative h-6 overflow-hidden rounded border border-purple-900 bg-[#0d1117]">
                {#if isThinking}
                    <div class="absolute inset-0 bg-gradient-to-r from-transparent via-purple-600 to-transparent animate-scan opacity-40"></div>
                {/if}
                
                <input class="absolute inset-0 w-full bg-transparent text-purple-200 px-2 py-0.5 text-[10px] outline-none placeholder-purple-900" 
                       placeholder={isThinking ? "GEMMA IS THINKING..." : "Type and hit Enter..."} 
                       bind:value={userPrompt} 
                       on:keydown={(e) => e.key === 'Enter' && sendNeuralPrompt()} 
                       disabled={isThinking} />
            </div>
        </div>

        <div class="flex items-center gap-4 hidden md:flex">
            <input class="bg-black border border-gray-700 text-gray-400 px-2 py-0.5 text-[10px] rounded w-48 focus:border-neon-green outline-none" 
                   bind:value={currentFile} on:keydown={(e) => e.key === 'Enter' && load()} />
            <button class="bg-neon-green text-black font-bold px-3 py-0.5 text-[10px] rounded hover:bg-white" on:click={save}>SAVE</button>
        </div>
    </header>

    <div class="flex-1 flex overflow-hidden">
        <aside class="w-64 border-r border-[#30363d] bg-[#0d1117] p-4 flex flex-col gap-6 hidden md:flex overflow-y-auto">
            <div>
                <div class="text-[10px] text-gray-500 mb-2 font-bold tracking-widest border-b border-[#30363d]">TELEMETRY</div>
                <Vitals ram={vitals.ram_usage} status={vitals.status} />
            </div>
            <div>
                <div class="text-[10px] text-purple-500 mb-2 font-bold tracking-widest border-b border-purple-900 flex justify-between">
                    <span>NEURAL_RESPONSE</span>
                    {#if isThinking}<span class="animate-pulse">THINKING...</span>{/if}
                </div>
                <div class="text-[9px] text-gray-400 whitespace-pre-wrap font-mono h-48 overflow-y-auto border border-[#30363d] p-2 bg-black">
                    {neuralResponse}
                </div>
            </div>
            <div>
                <div class="text-[10px] text-gray-500 mb-2 font-bold tracking-widest border-b border-[#30363d]">SWARM_RADAR</div>
                <AgentMap swarm={agents} />
            </div>
        </aside>

        <section class="flex-1 bg-black relative flex flex-col">
            <div class={activeTab === 'terminal' ? 'block h-full' : 'hidden h-full'}>
                <Terminal bind:this={terminalInstance} />
            </div>
            <div class={activeTab === 'editor' ? 'block h-full' : 'hidden h-full'}>
                <Editor bind:value={code} onChange={(val) => { code = val; isDirty = true; }} />
            </div>
        </section>
    </div>

    <footer class="h-6 shrink-0 bg-[#0d1117] border-t border-[#30363d] flex items-center px-2 text-[9px] text-gray-500 justify-between">
        <div class="flex gap-6">
             <span class="flex items-center gap-1">CPU: <span class="text-white">{vitals.cpu_usage.toFixed(1)}%</span></span>
             <span class="flex items-center gap-1">INT: <span class="text-white">{vitals.integrity}%</span></span>
             <span class="flex items-center gap-2 border-l border-gray-700 pl-4">
                LATENCY: <span class={neuralLatency > 5000 ? 'text-yellow-500 font-bold animate-pulse' : 'text-[#58a6ff]'}>{neuralLatency}ms</span>
             </span>
        </div>
        <span>NEXUS.V.0.18.0_DEV // VISUAL PULSE ACTIVE</span>
    </footer>
</div>

<style>
    @keyframes scan {
        0% { transform: translateX(-100%); }
        100% { transform: translateX(100%); }
    }
    .animate-scan {
        animation: scan 1.5s linear infinite;
        width: 100%;
        height: 100%;
    }
</style>
