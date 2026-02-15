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
  
  // Neural Link State
  let userPrompt = "";
  let neuralResponse = ">> WAITING FOR INPUT...";
  let neuralLatency = 0;
  let isThinking = false;

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

  // --- TELEMETRY & HEALTH ---
  async function updateVitals() {
      try {
          const res = await fetch('/api/vitals');
          if (res.ok) vitals = await res.json();
          else vitals.status = "ERROR";
      } catch (e) { vitals.status = "OFFLINE"; }
  }

  // --- NEURAL LINK (WITH SIGHT) ---
  async function sendNeuralPrompt() {
      if (!userPrompt) return;
      isThinking = true;
      neuralResponse = ">> UPLOADING CONTEXT TO GEMMA-3...";
      
      try {
          // SYNC FIX: Sending 'context' to match server.rs
          const res = await fetch('/api/prompt', {
              method: 'POST',
              headers: {'Content-Type': 'application/json'},
              body: JSON.stringify({ prompt: userPrompt, context: code })
          });
          
          if (res.ok) {
              const data = await res.json();
              neuralResponse = data.response;
              neuralLatency = data.latency; 
          } else {
              neuralResponse = ">> NEURAL_LINK_FAILURE";
          }
      } catch (e) {
          neuralResponse = ">> CRITICAL_ERROR: " + e.message;
      } finally {
          isThinking = false;
          userPrompt = ""; 
      }
  }

  onMount(() => {
    load();
    updateVitals();
    const i = setInterval(updateVitals, 2000);
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

        <div class="flex-1 max-w-xl flex items-center gap-2">
            <span class="text-[10px] text-purple-400 font-bold">GEMMA_LINK:</span>
            <input class="flex-1 bg-[#0d1117] border border-purple-900 text-purple-200 px-2 py-0.5 text-[10px] rounded focus:border-purple-500 outline-none placeholder-purple-900" 
                   placeholder="Ask about your code..." 
                   bind:value={userPrompt} 
                   on:keydown={(e) => e.key === 'Enter' && sendNeuralPrompt()} 
                   disabled={isThinking} />
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
                    {#if isThinking}<span class="animate-pulse">READING CODE...</span>{/if}
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
            {#if activeTab === 'editor'}
                <Editor bind:value={code} onChange={(val) => { code = val; isDirty = true; }} />
            {:else}
                <Terminal />
            {/if}
        </section>
    </div>

    <footer class="h-6 shrink-0 bg-[#0d1117] border-t border-[#30363d] flex items-center px-2 text-[9px] text-gray-500 justify-between">
        <div class="flex gap-6">
             <span class="flex items-center gap-1">CPU: <span class="text-white">{vitals.cpu_usage.toFixed(1)}%</span></span>
             <span class="flex items-center gap-1">INT: <span class="text-white">{vitals.integrity}%</span></span>
             
             <span class="flex items-center gap-2 border-l border-gray-700 pl-4">
                LATENCY: 
                <span class={neuralLatency > 5000 ? 'text-yellow-500 font-bold animate-pulse' : 'text-[#58a6ff]'}>
                    {neuralLatency}ms
                </span>
                {#if neuralLatency > 5000}
                    <span class="text-[8px] bg-yellow-900 text-yellow-200 px-1 rounded">HIGH LOAD</span>
                {/if}
             </span>
        </div>
        <span>NEXUS.V.0.15.0_BETA // NEURAL SIGHT ACTIVE</span>
    </footer>
</div>
