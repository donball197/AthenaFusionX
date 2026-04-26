<script lang="ts">
    import "./nexus_os.css";
    import { onMount } from 'svelte';
    import { fly } from 'svelte/transition';
    import { uplink } from '$lib/services/uplink.js';
    import { ide, layout, fileTreeTrigger } from '$lib/core/store.svelte';
    import { TaskService } from '$lib/services/TaskService';
    import { nexusSocket } from '$lib/core/socket';
    import { agentRegistry } from "$lib/core/agent_module_registry.svelte";

    // Components
    import NavHeader from '$lib/components/NavHeader.svelte';
    import GhostBlade from '$lib/components/GhostBlade.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import StatusBar from '$lib/components/StatusBar.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import ModuleRenderer from "$lib/components/ModuleRenderer.svelte";

    // Panels
    import IdePanel from '$lib/components/panels/IdePanel.svelte';
    import WorkspaceShell from '$lib/components/WorkspaceShell.svelte';
    import HistoryPanel from '$lib/components/panels/HistoryPanel.svelte';
    import VitalsView from '$lib/components/VitalsView.svelte';
    import MemoryVault from '$lib/components/MemoryVault.svelte';
    import SecOps from '$lib/components/SecOps.svelte';
    import AgentLab from '$lib/components/AgentLab.svelte';
    import CognitiveMesh from "$lib/components/CognitiveMesh.svelte";
    import TrinityMatrix from "$lib/components/TrinityMatrix.svelte";

    let activeNav = $state('NEXUS_OS');
    let openInput = $state('');
    let openError = $state('');
    let activeDynamicId = $state("");

    // Dynamic Component Map
    const PANELS: Record<string, any> = {
        'IDE': IdePanel,
        'WORKSPACE': WorkspaceShell,
        'VITALS': VitalsView,
        'MEMORY': MemoryVault,
        'SECOPS': SecOps,
        'AGENT-LAB': AgentLab,
        'HISTORY': HistoryPanel
    };

    onMount(() => {
        agentRegistry.propose({ 
            id: "sys_diag", 
            label: "Sys Diagnostics", 
            version: "1.0", 
            type: "panel", 
            render: { 
                kind: "terminal", 
                source: "Awaiting probe data...", 
                actions: [{ label: "RUN CARGO CHECK", directive: "run cargo check" }] 
            }, 
            generatedBy: "v7_bootstrap" 
        });
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
            const r = await uplink.fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
            if (!r.ok) { openError = `UPLINK_FAILURE: HTTP ${r.status}`; return; }
            const data = await r.json();
            ide.mountFile(path, data.content ?? '');
            activeNav = 'IDE';
            TaskService.log(`>> [IDE] Buffer Synchronized: ${path}`, 'text-emerald-400');
        } catch(e: any) {
            openError = e.message;
        }
    }

    async function createFile(path: string) {
        if (!path) return;
        try {
            const r = await uplink.post('/api/fs/write', { path, content: `// ${path}\n// Created by NEXUS IDE\n\n` });
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
                    <form onsubmit={(e) => {e.preventDefault(); openFile(openInput); openInput='';}} class="open-row">
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
            {:else if activeNav === "DYNAMIC_UI"}
                <div class="fill-panel flex-row">
                    <aside class="sidebar-col w-[200px] p-[10px] gap-2">
                        <div class="panel-section-hdr">DYNAMIC MODULES</div>
                        {#each agentRegistry.modules as m}
                            <button class="ws-btn" onclick={() => activeDynamicId = m.id} class:active={activeDynamicId === m.id}>◈ {m.label}</button>
                        {/each}
                    </aside>
                    <main class="flex-1 p-[10px]"><ModuleRenderer activeId={activeDynamicId} /></main>
                </div>
            {:else if activeNav === "COGNITIVE_MESH"}
                <div class="fill-panel flex-row gap-[10px] p-[10px]">
                    <div class="flex-1 border border-cyan-500/10"><CognitiveMesh /></div>
                    <div class="flex-1 border border-purple-500/20"><TrinityMatrix /></div>
                </div>
            {:else if PANELS[activeNav]}
                {@const ActivePanel = PANELS[activeNav]}
                <div class="fill-panel"><ActivePanel /></div>
            {:else}
                <div class="panel-placeholder">[{activeNav}_OFFLINE]</div>
            {/if}
        </main>
        <GhostBlade />
    </div>
    <StatusBar />
</div>

