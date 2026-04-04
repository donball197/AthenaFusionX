<script>
    import { activeTab, openTabs, layout, ghostBuffer, isGhostTyping } from "$lib/store";
    import { fly } from 'svelte/transition';
    import Editor from "./Editor.svelte";
    import Ghost from "./GhostCortex.svelte";
    import Sidebar from "./Sidebar.svelte";

    function toggleSidebar() {
        $layout.sidebarVisible = !$layout.sidebarVisible;
    }
</script>

<div class="forge-container relative flex flex-1 min-h-0 overflow-hidden">
    {#if !$layout.sidebarVisible}
        <button class="open-btn" transition:fly={{ x: -20, duration: 200 }} on:click={toggleSidebar}>
            <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l7 7-7 7M5 5l7 7-7 7"></path>
            </svg>
        </button>
    {/if}

    {#if $layout.sidebarVisible}
    <aside class="nexus-sidebar" transition:fly={{ x: -220, duration: 300, opacity: 1 }}>
        <div class="sidebar-header">
            <span class="sidebar-title">SECTOR_INDEX</span>
            <button class="toggle-btn" on:click={toggleSidebar}>
                <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7"></path>
                </svg>
            </button>
        </div>
        <div class="sidebar-content"><Sidebar /></div>
    </aside>
    {/if}

    <div class="nexus-ghost">
        <Ghost response={$ghostBuffer} active={$isGhostTyping} />
    </div>

    <div class="nexus-workspace flex-1 flex flex-col min-width-0 overflow-hidden">
        <div class="tabs" role="tablist">
            {#each $openTabs as t}
                <button class="tab {t.path === $activeTab ? 'active' : ''}" on:click={() => activeTab.set(t.path)}>
                    {#if t.isDirty}<span class="dirty-dot"></span>{/if}
                    {t.path.split('/').pop()}
                </button>
            {/each}
        </div>
        <div class="editor-host flex-1 relative overflow-hidden">
            {#if $activeTab}
                <Editor path={$activeTab} />
            {:else}
                <div class="welcome">AWAITING SECTOR SELECTION...</div>
            {/if}
        </div>
    </div>
</div>

<style>
    .forge-container { background: #060608; }
    .nexus-sidebar { width: 220px; flex-shrink: 0; border-right: 1px solid #1a1a1a; background: #0d1117; display: flex; flex-direction: column; overflow: hidden; z-index: 20; }
    .nexus-ghost { width: 320px; flex-shrink: 0; border-right: 1px solid #1a1a1a; background: #050508; display: flex; flex-direction: column; overflow: hidden; }
    .sidebar-header { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid #1a1a1a; background: #050508; }
    .sidebar-title { font-size: 10px; font-weight: bold; color: #22d3ee; letter-spacing: 2px; }
    .sidebar-content { flex: 1; overflow-y: auto; }
    .tabs { display: flex; background: #0d1117; border-bottom: 1px solid #1a1a1a; overflow-x: auto; flex-shrink: 0; }
    .tab { padding: 10px 16px; cursor: pointer; font-size: 11px; color: #666; border: none; border-right: 1px solid #1a1a1a; background: none; white-space: nowrap; }
    .tab.active { background: #000; color: #22d3ee; }
    .dirty-dot { width: 6px; height: 6px; background: #fbbf24; border-radius: 50%; display: inline-block; margin-right: 4px; }
    .welcome { height: 100%; display: flex; align-items: center; justify-content: center; color: #222; font-size: 14px; letter-spacing: 2px; text-transform: uppercase; }
    .open-btn { position: absolute; top: 8px; left: 8px; z-index: 50; background: #0d1117; border: 1px solid #1a1a1a; color: #22d3ee; cursor: pointer; padding: 4px; border-radius: 4px; }
    .toggle-btn { background: none; border: none; color: #22d3ee; cursor: pointer; }
</style>
