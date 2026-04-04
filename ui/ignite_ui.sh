#!/bin/bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1

# ==============================================================================
# NEXUS UI: AUTONOMOUS IGNITION SEQUENCE v0.14.0
# TARGET: Lenovo Duet 2 (ARM64) // SvelteKit Standardization
# ==============================================================================

UI_DIR=~/nexus_singularity/ui
echo -e "\n\033[1;36m>> [NEXUS] INITIATING UI STANDARDIZATION & UPGRADE...\033[0m"

# 1. CLEANUP & PREPARATION
# ------------------------
cd $UI_DIR
rm -rf .svelte-kit dist src/components src/App.svelte src/main.js src/app.htm
mkdir -p src/lib/components
mkdir -p src/routes

# 2. CONFIGURATION OVERHAUL
# -------------------------
echo ">> [CONFIG] Upgrading build matrix..."

# package.json - Standardized SvelteKit dependencies
cat > package.json << 'EOF'
{
  "name": "nexus-ui",
  "version": "0.14.0",
  "private": true,
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./jsconfig.json",
    "check:watch": "svelte-kit sync && svelte-check --tsconfig ./jsconfig.json --watch"
  },
  "devDependencies": {
    "@sveltejs/adapter-static": "^3.0.1",
    "@sveltejs/kit": "^2.5.0",
    "@sveltejs/vite-plugin-svelte": "^3.0.0",
    "autoprefixer": "^10.4.16",
    "postcss": "^8.4.32",
    "svelte": "^4.2.7",
    "svelte-check": "^3.6.0",
    "tailwindcss": "^3.3.6",
    "typescript": "^5.0.0",
    "vite": "^5.0.0"
  },
  "dependencies": {
    "@codemirror/commands": "^6.10.2",
    "@codemirror/lang-rust": "^6.0.2",
    "@codemirror/language": "^6.12.1",
    "@codemirror/state": "^6.5.4",
    "@codemirror/theme-one-dark": "^6.1.3",
    "@codemirror/view": "^6.39.14",
    "@lezer/highlight": "^1.2.3"
  },
  "type": "module"
}
EOF

# svelte.config.js - Configured for Static Adapter (SPA Mode)
cat > svelte.config.js << 'EOF'
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: 'dist',
			assets: 'dist',
			fallback: 'index.html', // Vital for SPA routing
			precompress: false,
			strict: true
		})
	},
	preprocess: vitePreprocess()
};

export default config;
EOF

# vite.config.js - SvelteKit Plugin & Proxy
cat > vite.config.js << 'EOF'
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		proxy: {
			'/api': 'http://localhost:3000',
			'/ws': {
				target: 'ws://localhost:3000',
				ws: true
			}
		}
	}
});
EOF

# tailwind.config.js - Pointing to Svelte files
cat > tailwind.config.js << 'EOF'
/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      colors: {
        'neon-green': '#00ff41',
        'cyber-black': '#050505'
      },
      fontFamily: {
        mono: ['"Fira Code"', 'monospace']
      }
    },
  },
  plugins: [],
}
EOF

# 3. COMPONENT RESTORATION
# ------------------------
echo ">> [COMPONENTS] Restoring Neural Interface Modules..."

# Editor.svelte
cat > src/lib/components/Editor.svelte << 'EOF'
<script>
    import { onMount } from 'svelte';
    import { EditorView, basicSetup } from "codemirror";
    import { EditorState } from "@codemirror/state";
    import { rust } from "@codemirror/lang-rust";
    import { oneDark } from "@codemirror/theme-one-dark";
    
    export let value = "";
    export let onChange = () => {};
    
    let editorElement;
    let view;

    onMount(() => {
        let state = EditorState.create({
            doc: value,
            extensions: [
                basicSetup,
                rust(),
                oneDark,
                EditorView.updateListener.of((u) => {
                    if (u.docChanged) {
                        const val = u.state.doc.toString();
                        onChange(val);
                    }
                })
            ]
        });

        view = new EditorView({
            state,
            parent: editorElement
        });

        return () => view.destroy();
    });

    $: if (view && value !== view.state.doc.toString()) {
        view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: value }
        });
    }
</script>

<div class="h-full w-full overflow-hidden text-sm" bind:this={editorElement}></div>
EOF

# Terminal.svelte
cat > src/lib/components/Terminal.svelte << 'EOF'
<script>
    import { onMount } from 'svelte';
    let termElement;
    let socket;
    let output = [">> SECURE CHANNEL OPENED", ">> WAITING FOR KERNEL..."];

    onMount(() => {
        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        socket = new WebSocket(`${proto}//${window.location.host}/ws`);

        socket.onmessage = (event) => {
            output = [...output, event.data];
            // Keep only last 100 lines to save RAM
            if (output.length > 100) output = output.slice(-100);
            scrollToBottom();
        };

        return () => socket.close();
    });

    function scrollToBottom() {
        if (termElement) termElement.scrollTop = termElement.scrollHeight;
    }

    function handleInput(e) {
        if (e.key === 'Enter') {
            const cmd = e.target.value;
            socket.send(cmd + '\n');
            e.target.value = '';
        }
    }
</script>

<div class="h-full flex flex-col bg-black text-green-500 font-mono p-2 overflow-hidden">
    <div class="flex-1 overflow-y-auto whitespace-pre-wrap" bind:this={termElement}>
        {#each output as line}
            <div>{line}</div>
        {/each}
    </div>
    <div class="flex gap-2 mt-2 border-t border-gray-800 pt-2">
        <span class="text-neon-green">root@nexus:~#</span>
        <input 
            class="flex-1 bg-transparent border-none outline-none text-white focus:ring-0" 
            on:keydown={handleInput}
            autofocus
        />
    </div>
</div>
EOF

# Vitals.svelte
cat > src/lib/components/Vitals.svelte << 'EOF'
<script>
    export let ram = "0MB";
    export let status = "OFFLINE";
    $: ramPercent = (parseInt(ram) / 2000) * 100;
</script>

<div class="bg-[#050505] border border-[#1a1a1a] p-4 font-mono text-xs">
    <div class="flex justify-between mb-2">
        <span class="text-gray-500">SYSTEM_STATUS</span>
        <span class={status === 'ONLINE' ? 'text-neon-green' : 'text-red-500'}>{status}</span>
    </div>
    <div class="mb-1 text-gray-400">RAM_USAGE: {ram}</div>
    <div class="h-1 w-full bg-[#111]">
        <div class="h-full bg-neon-green transition-all duration-500" style="width: {ramPercent}%"></div>
    </div>
</div>
EOF

# AgentMap.svelte
cat > src/lib/components/AgentMap.svelte << 'EOF'
<script>
    export let swarm = [];
    function getCoords(target) {
        if (!target || target === "IDLE") return { x: 50, y: 50, active: false };
        let hash = 0;
        for (let i = 0; i < target.length; i++) hash = target.charCodeAt(i) + ((hash << 5) - hash);
        return { x: Math.abs((hash % 80) + 10), y: Math.abs(((hash >> 8) % 80) + 10), active: true };
    }
</script>
<div class="relative h-[200px] bg-black border border-[#1a1a1a] overflow-hidden mb-4">
    <div class="absolute inset-0 bg-[radial-gradient(#111_1px,transparent_1px)] [background-size:20px_20px]"></div>
    <div class="absolute w-full h-[2px] bg-gradient-to-b from-transparent via-green-500 to-transparent opacity-10 animate-[scan_4s_linear_infinite]"></div>
    {#each swarm as agent}
        {@const pos = getCoords(agent.target)}
        <div class="absolute w-1 h-1 bg-[#222] rounded-full transition-all duration-700" 
             class:bg-neon-green={pos.active} 
             class:shadow-[0_0_8px_#0f0]={pos.active}
             style="left: {pos.x}%; top: {pos.y}%;"></div>
    {/each}
</div>
EOF

# 4. ROUTING LOGIC
# ----------------
echo ">> [ROUTING] Wiring Neural Pathways..."

# +layout.svelte (Global Styles)
cat > src/routes/+layout.svelte << 'EOF'
<script>
  import '../app.css';
</script>
<slot />
EOF

# +page.svelte (Main Dashboard)
cat > src/routes/+page.svelte << 'EOF'
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
  
  let vitals = { status: "OFFLINE", cpu_usage: 0, ram_usage: "0MB", integrity: 100 };
  let agents = [{id: 1, target: "main.rs"}, {id: 2, target: "IDLE"}];

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

  async function updateVitals() {
      try {
          const res = await fetch('/api/vitals');
          if (res.ok) vitals = await res.json();
          else vitals.status = "ERROR";
      } catch (e) { vitals.status = "OFFLINE"; }
  }

  onMount(() => {
    load();
    updateVitals();
    const i = setInterval(updateVitals, 2000);
    return () => clearInterval(i);
  });
</script>

<div class="flex flex-col h-screen w-screen overflow-hidden bg-black text-green-500 font-mono">
    <!-- HEADER -->
    <header class="h-10 shrink-0 bg-[#161b22] border-b border-[#30363d] flex items-center px-4 justify-between">
        <div class="flex items-center gap-6">
            <span class="text-[#58a6ff] font-bold text-xs tracking-widest">NEXUS // VISOR</span>
            <div class="flex gap-1">
                <button on:click={() => activeTab = 'editor'} class="text-[9px] px-3 py-1 border transition-colors {activeTab === 'editor' ? 'border-[#58a6ff] bg-[#1f242c] text-white' : 'border-[#30363d] text-gray-500'}">EDITOR</button>
                <button on:click={() => activeTab = 'terminal'} class="text-[9px] px-3 py-1 border transition-colors {activeTab === 'terminal' ? 'border-[#58a6ff] bg-[#1f242c] text-white' : 'border-[#30363d] text-gray-500'}">TERMINAL</button>
            </div>
        </div>
        <div class="flex items-center gap-4">
            <input class="bg-black border border-gray-700 text-gray-400 px-2 py-0.5 text-[10px] rounded w-64 focus:border-neon-green outline-none" 
                   bind:value={currentFile} on:keydown={(e) => e.key === 'Enter' && load()} />
            <button class="bg-neon-green text-black font-bold px-3 py-0.5 text-[10px] rounded hover:bg-white" on:click={save}>SAVE</button>
        </div>
    </header>

    <!-- MAIN BODY -->
    <div class="flex-1 flex overflow-hidden">
        <!-- SIDEBAR -->
        <aside class="w-64 border-r border-[#30363d] bg-[#0d1117] p-4 flex flex-col gap-6 hidden md:flex">
            <div>
                <div class="text-[10px] text-gray-500 mb-2 font-bold tracking-widest border-b border-[#30363d]">TELEMETRY</div>
                <Vitals ram={vitals.ram_usage} status={vitals.status} />
            </div>
            <div>
                <div class="text-[10px] text-gray-500 mb-2 font-bold tracking-widest border-b border-[#30363d]">SWARM_RADAR</div>
                <AgentMap swarm={agents} />
            </div>
        </aside>

        <!-- CONTENT -->
        <section class="flex-1 bg-black relative flex flex-col">
            {#if activeTab === 'editor'}
                <Editor bind:value={code} onChange={(val) => { code = val; isDirty = true; }} />
            {:else}
                <Terminal />
            {/if}
        </section>
    </div>

    <!-- FOOTER -->
    <footer class="h-5 shrink-0 bg-[#0d1117] border-t border-[#30363d] flex items-center px-2 text-[9px] text-gray-500 justify-between">
        <div class="flex gap-4">
             <span>CPU: {vitals.cpu_usage}%</span>
             <span>INT: {vitals.integrity}%</span>
        </div>
        <span>NEXUS.V.0.14.0_STABLE</span>
    </footer>
</div>
EOF

# 5. EXECUTE BUILD
# ----------------
echo ">> [IGNITION] Installing dependencies & Building..."
npm install
npm run build

echo -e "\n\033[1;32m>> [NEXUS] UI UPGRADE COMPLETE.\033[0m"
echo ">> [INFO] Build artifacts are located in 'ui/dist'."
echo ">> [INFO] The Rust backend will now serve this dashboard automatically."
