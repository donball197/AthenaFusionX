<script lang="ts">
import { ide, layout, fileTreeTrigger } from '$lib/core/store.svelte.js';
import { TaskService } from '$lib/services/TaskService.js';
import { fly } from 'svelte/transition';
import { onMount } from 'svelte';

import NavHeader from '$lib/components/NavHeader.svelte';
import GhostBlade from '$lib/components/GhostBlade.svelte';
import Sidebar from '$lib/components/Sidebar.svelte';
import StatusBar from '$lib/components/StatusBar.svelte';
import Terminal from '$lib/components/Terminal.svelte';

// Extracted Panels
import IdePanel from '$lib/components/panels/IdePanel.svelte';
import WorkspacePanel from '$lib/components/panels/WorkspacePanel.svelte';
import HistoryPanel from '$lib/components/panels/HistoryPanel.svelte';
import VitalsView from '$lib/components/VitalsView.svelte';
import MemoryVault from '$lib/components/MemoryVault.svelte';
import SecOps from '$lib/components/SecOps.svelte';
import AgentLab from '$lib/components/AgentLab.svelte';
import CognitiveMesh from "$lib/components/CognitiveMesh.svelte";
import TrinityMatrix from "$lib/components/TrinityMatrix.svelte";
import ModuleRenderer from "$lib/components/ModuleRenderer.svelte";
import { agentRegistry } from "$lib/core/agent_module_registry.svelte.js";
import { nexusSocket } from '$lib/nexus_socket.svelte.ts';

let activeNav = $state('NEXUS_OS');
let openInput = $state('');
let openError = $state('');
let activeDynamicId = $state("");

onMount(() => {
    // ⚡ V7 BOOTSTRAP
    agentRegistry.propose({ id: "sys_diag", label: "Sys Diagnostics", version: "1.0", type: "panel", render: { kind: "terminal", source: "Awaiting probe data...", actions: [{ label: "RUN CARGO CHECK", directive: "run cargo check" }] }, generatedBy: "v7_bootstrap" });
    TaskService.log('>> [NEXUS] Singularity OS v4.3.0 — Substrate Initialized', 'text-cyan-400');
});

async function openFile(path: string) {
    if (!path?.trim()) return;
    openError = '';
    if (ide.openTabs.some(t => t.path === path)) {
        ide.activeTab = path;
        activeNav = 'IDE';
        return;
    }
    try {
        const r = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
        if (!r.ok) { openError = `UPLINK_FAILURE: HTTP ${r.status}`; return; }
        const data = await r.json();
        ide.mountFile(path, data.content ?? '');
        activeNav = 'IDE';
        TaskService.log(`>> [IDE] Buffer Synchronized: ${path}`, 'text-emerald-400');
    } catch(e: any) {
        openError = e.message;
    }
}

async function submitOpenInput(e: Event) {
    e.preventDefault();
    const p = openInput.trim();
    if (p) { await openFile(p); openInput = ''; }
}

async function createFile(path: string) {
    if (!path) return;
    try {
        const r = await fetch('/api/fs/write', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path, content: `// ${path}\n// Created by NEXUS IDE\n\n` })
        });
        if (r.ok) { fileTreeTrigger.value++; await openFile(path); }
    } catch(e: any) {
        TaskService.log(`!! [IDE] FRACTURE: ${e.message}`, 'text-rose-400');
    }
}
</script>

<div class="nexus-root">
    <NavHeader bind:activeNav />

    <div class="body-row">
        {#if layout.explorerVisible}
            <aside class="sidebar-col" transition:fly={{ x: -224, duration: 160, opacity: 1 }}>
                <div class="open-box">
                    <div class="open-label">VIRTUAL_FS_LINK</div>
                    <form onsubmit={submitOpenInput} class="open-row">
                        <input bind:value={openInput} placeholder="src/main.rs" class="open-inp" autocomplete="off" />
                        <button type="submit" class="open-go">↵</button>
                    </form>
                    {#if openError}<div class="open-err">{openError}</div>{/if}
                </div>
                <Sidebar onFileSelect={openFile} onCreateFile={createFile} />
            </aside>
        {/if}

        <main class="main-col">
            {#if activeNav === 'NEXUS_OS'}
                <div class="panel-scroll nexus-os-panel">
                    <div class="os-hero">
                        <div class="os-hero-glyph">◈</div>
                        <div class="os-hero-text">
                            <div class="os-hero-title">NEXUS SINGULARITY</div>
                            <div class="os-hero-sub">ARM64_SUBSTRATE // CROSTINI_VIRTUALIZATION</div>
                        </div>
                    </div>
                    <div class="os-grid">
                        <div class="os-card">
                            <div class="os-card-hdr">CORE_ARCHITECTURE</div>
                            <div class="os-node-grid">
                                {#each ['AXUM_SRV','GHOST_CRTX','ZIG_MUSCLE','VMO_POOL','LAZARUS','ARBITER'] as node}
                                    <div class="os-node">
                                        <span class="os-node-dot"></span>
                                        <span class="os-node-lbl">{node}</span>
                                    </div>
                                {/each}
                            </div>
                        </div>
                        <div class="os-card">
                            <div class="os-card-hdr">SUBSTRATE_TELEMETRY</div>
                            <div class="vmo-kv"><span>STATUS</span><span style="color:#22d3ee">{nexusSocket.telemetry?.status || 'IDLE'}</span></div>
                            <div class="vmo-kv"><span>ACTIVE VMOs</span><span style="color:#34d399">{nexusSocket.telemetry?.active_vmos || 0}</span></div>
                            <div class="vmo-kv"><span>RESONANCE</span><span style="color:#fbbf24">{(nexusSocket.telemetry?.resonance || 0).toFixed(2)}</span></div>
                        </div>
                    </div>
                    <div class="os-term-wrap">
                        <div class="panel-section-hdr">NEURAL_OUTPUT_STREAM</div>
                        <Terminal />
                    </div>
                </div>

            {:else if activeNav === 'IDE'}
                <IdePanel />
            {:else if activeNav === 'WORKSPACE'}
                <WorkspacePanel />
            {:else if activeNav === 'VITALS'}
                <div class="fill-panel"><VitalsView /></div>
            {:else if activeNav === 'MEMORY'}
                <div class="fill-panel"><MemoryVault /></div>
            {:else if activeNav === 'SECOPS'}
                <div class="fill-panel"><SecOps /></div>
            {:else if activeNav === 'AGENT-LAB'}
                <div class="fill-panel"><AgentLab /></div>
            {:else if activeNav === "DYNAMIC_UI"}
                <div class="fill-panel" style="flex-direction:row;">
                    <aside class="sidebar-col" style="width:200px; padding:10px; gap:8px;">
                        <div class="panel-section-hdr">DYNAMIC MODULES</div>
                        {#each agentRegistry.modules as m}
                            <button class="ws-btn" onclick={() => activeDynamicId = m.id} style={activeDynamicId === m.id ? "border-color:#00ffff; color:#00ffff; background:rgba(0,255,255,0.05);" : ""}>◈ {m.label}</button>
                        {/each}
                    </aside>
                    <main style="flex:1; padding:10px;"><ModuleRenderer activeId={activeDynamicId} /></main>
                </div>
            {:else if activeNav === "COGNITIVE_MESH"}
                <div class="fill-panel" style="flex-direction: row; gap: 10px; padding: 10px;">
                    <div style="flex: 1; border: 1px solid rgba(0,255,255,0.1);"><CognitiveMesh /></div>
                    <div style="flex: 1; border: 1px solid rgba(167,139,250,0.2);"><TrinityMatrix /></div>
                </div>
            {:else if activeNav === 'HISTORY'}
                <HistoryPanel />
            {:else}
                <div class="panel-placeholder">[{activeNav}_OFFLINE]</div>
            {/if}
        </main>
        <GhostBlade />
    </div>
    <StatusBar />
</div>

<style>
:global(html, body) { margin:0; padding:0; height:100vh; background:#000; overflow:hidden; color-scheme:dark; }
:global(*) { box-sizing:border-box; }
.nexus-root { position:fixed; inset:0; display:flex; flex-direction:column; font-family:'JetBrains Mono',monospace; background:#000; color:#e2e8f0; }
.body-row { flex:1; display:flex; overflow:hidden; min-height:0; }
.main-col { flex:1; display:flex; flex-direction:column; overflow:hidden; min-width:0; }
.fill-panel { flex:1; overflow:hidden; display:flex; flex-direction:column; }
.panel-scroll { flex:1; overflow-y:auto; padding:18px; }
.sidebar-col { width:224px; border-right:1px solid rgba(0,255,255,.07); background:#020208; flex:none; display:flex; flex-direction:column; }

.open-box { flex:none; padding:8px; border-bottom:1px solid rgba(0,255,255,.05); }
.open-label { font-size:7px; font-weight:900; letter-spacing:.14em; color:rgba(0,255,255,.25); margin-bottom:4px; }
.open-row { display:flex; gap:3px; }
.open-inp { flex:1; background:#000; border:1px solid rgba(0,255,255,.12); color:#22d3ee; font-size:10px; padding:4px 7px; outline:none; font-family:'JetBrains Mono',monospace; }
.open-go { width:28px; background:rgba(34,211,238,.1); border:1px solid rgba(0,255,255,.2); color:#22d3ee; font-size:14px; cursor:pointer; }
.open-err { font-size:8px; color:#f87171; margin-top:3px; }

.os-hero { padding:14px; border:1px solid rgba(0,255,255,.1); background:rgba(0,255,255,.02); margin-bottom:20px; display:flex; align-items:center; gap:14px; }
.os-hero-glyph { font-size:28px; color:#00ffff; opacity:.4; }
.os-hero-title { font-size:16px; font-weight:900; color:#00ffff; text-shadow:0 0 15px rgba(0,255,255,.4); }
.os-hero-sub { font-size:8px; color:rgba(0,255,255,.3); margin-top:3px; }
.os-grid { display:grid; grid-template-columns:1fr 1fr; gap:10px; margin-bottom:20px; }
.os-card { border:1px solid rgba(0,255,255,.07); padding:12px; background:rgba(0,255,255,.01); }
.os-card-hdr { font-size:8px; font-weight:900; letter-spacing:.14em; color:rgba(0,255,255,.3); margin-bottom:10px; }
.os-node-grid { display:flex; flex-direction:column; gap:5px; }
.os-node { display:flex; align-items:center; gap:6px; font-size:9px; }
.os-node-dot { width:5px; height:5px; border-radius:50%; background:#00ffff; box-shadow:0 0 5px #00ffff; flex:none; }
.os-node-lbl { color:rgba(0,255,255,.5); }
.vmo-kv { display:flex; justify-content:space-between; font-size:9px; padding:4px 0; border-bottom:1px solid rgba(0,255,255,.04); }
.vmo-kv span:first-child { color:rgba(255,255,255,.3); }
.panel-section-hdr { font-size:8px; font-weight:900; letter-spacing:.14em; color:rgba(0,255,255,.25); margin-bottom:8px; }
.os-term-wrap { border:1px solid rgba(0,255,255,.06); height:280px; display:flex; flex-direction:column; overflow:hidden; }
.ws-btn { width:100%; text-align:left; padding:7px 10px; background:transparent; border:1px solid rgba(0,255,255,.1); color:#94a3b8; font-size:9px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .1s; }
.ws-btn:hover { color:#00ffff; border-color:rgba(0,255,255,.4); background:rgba(0,255,255,.04); }
.panel-placeholder { flex:1; display:flex; align-items:center; justify-content:center; font-size:14px; opacity:.1; letter-spacing:10px; }
</style>
