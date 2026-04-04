<script>
    import { onMount } from "svelte";
    import { WS } from "$lib/components/NexusWS";
    import { 
        activeTab, openTabs, layout, 
        ghostBuffer, isGhostTyping 
    } from "$lib/store";
    import { fly } from 'svelte/transition';
    
    import Editor      from "$lib/components/Editor.svelte";
    import Terminal    from "$lib/components/Terminal.svelte";
    import Ghost       from "$lib/components/GhostCortex.svelte";
    import Sidebar     from "$lib/components/Sidebar.svelte";
    import StatusBar   from "$lib/components/StatusBar.svelte";
    import MergeGate   from "$lib/components/MergeGate.svelte";

    onMount(() => WS.connect());

    function toggleSidebar() {
        $layout.sidebarVisible = !$layout.sidebarVisible;
    }
</script>

<div class="nexus-root">
    <div class="nexus-main relative">
        
        {#if !$layout.sidebarVisible}
            <button 
                class="toggle-btn open-btn" 
                transition:fly={{ x: -20, duration: 200 }} 
                on:click={toggleSidebar} 
                title="Expand Sector Index"
            >
                <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l7 7-7 7M5 5l7 7-7 7"></path>
                </svg>
            </button>
        {/if}

        {#if $layout.sidebarVisible}
        <aside 
            class="nexus-sidebar"
            transition:fly={{ x: -220, duration: 300, opacity: 1 }}
        >
            <div class="sidebar-header">
                <span class="sidebar-title">SECTOR_INDEX</span>
                <button class="toggle-btn" on:click={toggleSidebar} title="Retract Explorer">
                    <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7"></path>
                    </svg>
                </button>
            </div>

            <div class="sidebar-content">
                <Sidebar />
            </div>
        </aside>
        {/if}

        <div class="nexus-ghost">
            <Ghost 
                response={$ghostBuffer} 
                active={$isGhostTyping} 
            />
        </div>

        <div class="nexus-workspace">
            <div class="nexus-editor">
                <div class="tabs" role="tablist">
                    {#each $openTabs as t}
                        <button 
                            class="tab {t.path === $activeTab ? 'active' : ''}" 
                            on:click={() => activeTab.set(t.path)} 
                            role="tab" 
                            aria-selected={t.path === $activeTab}>
                            {#if t.isDirty}<span class="dirty-dot"></span>{/if}
                            {t.path.split('/').pop()}
                        </button>
                    {/each}
                </div>
                <div class="editor-host">
                    {#if $activeTab}
                        <Editor path={$activeTab} />
                    {:else}
                        <div class="welcome">AWAITING SECTOR SELECTION...</div>
                    {/if}
                </div>
            </div>
            
            <div class="nexus-terminal">
                <Terminal />
            </div>
        </div>
    </div>
    
    <StatusBar />
    <MergeGate />
</div>

<style>
    .nexus-root { display: flex; flex-direction: column; height: 100vh; width: 100vw; overflow: hidden; background: #060608; color: #eee; font-family: 'Inter', sans-serif; }
    .nexus-main { display: flex; flex: 1; min-height: 0; overflow: hidden; position: relative; }
    
    /* Optimized widths for 4GB Duet 2 */
    .nexus-sidebar { width: 220px; flex-shrink: 0; border-right: 1px solid #1a1a1a; background: #0d1117; display: flex; flex-direction: column; overflow: hidden; z-index: 20; }
    .nexus-ghost { width: 320px; flex-shrink: 0; border-right: 1px solid #1a1a1a; background: #050508; display: flex; flex-direction: column; overflow: hidden; }
    .nexus-workspace { flex: 1; display: flex; flex-direction: column; min-width: 0; overflow: hidden; }
    
    .nexus-editor { flex: 2; display: flex; flex-direction: column; min-height: 0; background: #000; }
    .editor-host { flex: 1; overflow: hidden; position: relative; }
    .nexus-terminal { flex: 1; border-top: 1px solid #1a1a1a; background: #050505; display: flex; flex-direction: column; min-height: 0; }

    .tabs { display: flex; background: #0d1117; border-bottom: 1px solid #1a1a1a; overflow-x: auto; scrollbar-width: none; flex-shrink: 0; }
    .tab { display: flex; align-items: center; gap: 6px; padding: 10px 16px; cursor: pointer; font-size: 11px; color: #666; border: none; border-right: 1px solid #1a1a1a; background: none; white-space: nowrap; outline: none; position: relative; transition: color 0.2s; }
    .tab:hover { color: #aaa; }
    .tab.active { background: #000; color: #22d3ee; }
    
    .dirty-dot { width: 6px; height: 6px; background: #fbbf24; border-radius: 50%; box-shadow: 0 0 5px #fbbf24; }
    .welcome { height: 100%; display: flex; align-items: center; justify-content: center; color: #222; font-size: 14px; letter-spacing: 2px; text-transform: uppercase; }

    /* New Visor Toggle Styles */
    .sidebar-header { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid #1a1a1a; background: #050508; }
    .sidebar-title { font-size: 10px; font-weight: bold; color: #22d3ee; letter-spacing: 2px; }
    .sidebar-content { flex: 1; overflow-y: auto; scrollbar-width: thin; scrollbar-color: #22d3ee transparent; }
    
    .toggle-btn { background: none; border: none; color: #22d3ee; cursor: pointer; padding: 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center; transition: all 0.2s; }
    .toggle-btn:hover { background: rgba(34, 211, 238, 0.1); color: #fff; }
    .open-btn { position: absolute; top: 8px; left: 8px; z-index: 50; background: #0d1117; border: 1px solid #1a1a1a; box-shadow: 0 0 10px rgba(0,0,0,0.5); }
</style>
