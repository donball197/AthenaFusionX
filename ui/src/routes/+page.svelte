/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


<script>
    import { onMount } from "svelte";
    import { connectWS } from "$lib/core/NexusWS.js";
    import { ide, terminalState, systemVitals, layout } from '$lib/core/store.svelte.js';
    import { fly } from 'svelte/transition';
    
    import Editor      from "$lib/components/Editor.svelte";
    import Terminal    from "$lib/components/Terminal.svelte";
    import Ghost       from "$lib/components/GhostCortex.svelte";
    import Sidebar     from "$lib/components/Sidebar.svelte";
    import StatusBar   from "$lib/components/StatusBar.svelte";
    import MergeGate   from "$lib/components/MergeGate.svelte";
    
    import VitalsView  from "$lib/components/VitalsView.svelte";
    import SecOps      from "$lib/components/SecOps.svelte";
    import AgentLab    from "$lib/components/AgentLab.svelte";

    let activeNav = $state('WORKSPACE');
    const navItems = ['NEXUS_OS', 'IDE', 'SECOPS', 'WORKSPACE', 'VITALS', 'ASSET', 'AGENT-LAB', 'HISTORY'];

    onMount(() => connectWS());

    function toggleSidebar() { layout.sidebarVisible = !layout.sidebarVisible; }
    function toggleVisor() { layout.visorVisible = !layout.visorVisible; }

    function closeTab(e, path) {
        e.stopPropagation();
        ide.openTabs = ide.openTabs.filter(t => t.path !== path);
        if (ide.activeTab === path) {
            ide.activeTab = ide.openTabs.length > 0 ? ide.openTabs[ide.openTabs.length - 1].path : null;
        }
    }
</script>

<svelte:head><title>NEXUS SINGULARITY</title></svelte:head>

<div class="nexus-root">
    <header class="flex-none h-10 border-b border-[#1f2937] flex items-center justify-between px-4 bg-[#0a0a0a]">
        <div class="flex h-full gap-1 items-center">
            {#each navItems as item}
                <button 
                    class="h-full px-4 text-[10px] uppercase tracking-widest font-bold transition-all {activeNav === item ? 'bg-cyan-400 text-black shadow-[0_0_10px_rgba(34,211,238,0.2)]' : 'text-cyan-600 hover:text-cyan-400 hover:bg-cyan-900/20'}"
                    onclick={() => activeNav = item}>
                    {item}
                </button>
            {/each}
        </div>
        <div class="flex gap-4 text-[9px] uppercase tracking-widest text-gray-500 font-bold">
            <span>CPU <span class="text-cyan-400">{(systemVitals.cpu_load * 100).toFixed(0)}%</span></span>
            <span>RAM <span class="text-purple-400">{(systemVitals.ram_gb || 0).toFixed(2)}GB</span></span>
            <span>TEMP <span class="text-orange-400">{(systemVitals.temperature || 0).toFixed(1)}°C</span></span>
            <span>RES <span class="text-emerald-400">{(systemVitals.resonance || 0).toFixed(2)}</span></span>
        </div>
    </header>

    <div class="nexus-main relative">
        {#if !layout.sidebarVisible && activeNav === 'WORKSPACE'}
            <button class="toggle-btn open-btn" transition:fly={{ x: -20, duration: 200 }} onclick={toggleSidebar} aria-label="Open Sidebar">
                <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l7 7-7 7M5 5l7 7-7 7"></path></svg>
            </button>
        {/if}

        {#if layout.sidebarVisible && activeNav === 'WORKSPACE'}
            <aside class="nexus-sidebar" transition:fly={{ x: -220, duration: 300, opacity: 1 }}>
                <div class="sidebar-header">
                    <span class="sidebar-title">SECTOR_INDEX</span>
                    <button class="toggle-btn" onclick={toggleSidebar} aria-label="Close Sidebar"><svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7"></path></svg></button>
                </div>
                <div class="sidebar-content"><Sidebar /></div>
            </aside>
        {/if}

        <div class="nexus-workspace">
            {#if activeNav === 'WORKSPACE'}
                <div class="nexus-editor">
                    <div class="tabs" role="tablist">
                        <div class="flex flex-1 overflow-x-auto">
                            {#each ide.openTabs as t}
                                <div class="tab {t.path === ide.activeTab ? 'active' : ''}" 
                                     role="button" 
                                     tabindex="0"
                                     onclick={() => ide.activeTab = t.path}
                                     onkeydown={(e) => e.key === 'Enter' && (ide.activeTab = t.path)}>
                                    {#if t.isDirty}<span class="dirty-dot"></span>{/if}
                                    {t.path.split('/').pop()}
                                    <button class="ml-2 opacity-40 hover:opacity-100" onclick={(e) => closeTab(e, t.path)} aria-label="Close Tab">×</button>
                                </div>
                            {/each}
                        </div>
                        <div class="flex items-center px-4 gap-4 text-[9px] font-black text-cyan-800">
                             <button onclick={toggleSidebar} class="hover:text-cyan-400">{layout.sidebarVisible ? '◀' : '▶'} DNA_FS</button>
                             <button onclick={toggleVisor} class="hover:text-cyan-400">{layout.visorVisible ? '▶' : '◀'} VISOR_LINK</button>
                        </div>
                    </div>
                    <div class="editor-host">
                        {#if ide.activeTab} <Editor path={ide.activeTab} /> {:else} <div class="welcome">AWAITING SECTOR SELECTION...</div> {/if}
                    </div>
                </div>
                <div class="nexus-terminal border-t border-[#1a1a1a]"><Terminal /></div>
            {:else if activeNav === 'VITALS'}
                <div class="flex-1 bg-black overflow-y-auto"><VitalsView /></div>
            {:else if activeNav === 'SECOPS'}
                <div class="flex-1 p-4 bg-black"><SecOps /></div>
            {:else if activeNav === 'AGENT-LAB'}
                <div class="flex-1 bg-black"><AgentLab /></div>
            {:else}
                <div class="flex-1 flex items-center justify-center text-cyan-900 font-black text-2xl uppercase">[{activeNav} MODULE STANDBY]</div>
            {/if}
        </div>

        {#if layout.visorVisible}
            <aside class="nexus-ghost" transition:fly={{ x: 320, duration: 300, opacity: 1 }}>
                <div class="sidebar-header">
                    <span class="sidebar-title">GHOST_CORTEX</span>
                    <button class="toggle-btn" onclick={toggleVisor} aria-label="Close Visor"><svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l-7 7 7 7M19 5l-7 7 7 7"></path></svg></button>
                </div>
                <div class="sidebar-content"><Ghost response={terminalState.ghostBuffer} active={terminalState.isGhostTyping} /></div>
            </aside>
        {/if}
    </div>
    <StatusBar />
    <MergeGate />
</div>

<style>
    .nexus-root { display: flex; flex-direction: column; height: 100vh; width: 100vw; overflow: hidden; background: #060608; color: #eee; font-family: 'Inter', sans-serif; }
    .nexus-main { display: flex; flex: 1; min-height: 0; overflow: hidden; position: relative; }
    .nexus-sidebar { width: 220px; flex-shrink: 0; border-right: 1px solid #1a1a1a; background: #0d1117; display: flex; flex-direction: column; z-index: 20; }
    .nexus-ghost { width: 320px; flex-shrink: 0; border-left: 1px solid #1a1a1a; background: #050508; display: flex; flex-direction: column; z-index: 20; }
    .nexus-workspace { flex: 1; display: flex; flex-direction: column; min-width: 0; overflow: hidden; }
    .nexus-editor { flex: 2; display: flex; flex-direction: column; min-height: 0; background: #000; }
    .editor-host { flex: 1; overflow: hidden; position: relative; }
    .nexus-terminal { height: 35%; border-top: 1px solid #1a1a1a; background: #050505; display: flex; flex-direction: column; min-height: 250px; }
    .tabs { display: flex; background: #0d1117; border-bottom: 1px solid #1a1a1a; overflow-x: auto; flex-shrink: 0; }
    .tab { display: flex; align-items: center; gap: 6px; padding: 10px 16px; cursor: pointer; font-size: 11px; color: #666; border-right: 1px solid #1a1a1a; white-space: nowrap; }
    .tab.active { background: #000; color: #22d3ee; border-top: 1px solid #22d3ee; }
    .dirty-dot { width: 6px; height: 6px; background: #fbbf24; border-radius: 50%; }
    .welcome { height: 100%; display: flex; align-items: center; justify-content: center; color: #222; font-size: 14px; letter-spacing: 2px; text-transform: uppercase; }
    .sidebar-header { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid #1a1a1a; background: #050508; }
    .sidebar-title { font-size: 10px; font-weight: bold; color: #22d3ee; letter-spacing: 2px; }
    .sidebar-content { flex: 1; overflow-y: auto; }
    .toggle-btn { background: none; border: none; color: #22d3ee; cursor: pointer; padding: 4px; display: flex; align-items: center; }
    .open-btn { position: absolute; top: 50px; left: 8px; z-index: 50; background: #0d1117; border: 1px solid #1a1a1a; }
</style>
