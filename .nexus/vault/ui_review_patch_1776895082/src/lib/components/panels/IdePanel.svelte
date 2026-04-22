<script>
    import { uplink } from '$lib/services/uplink.js';
    import { ide } from '$lib/core/store.svelte.js';
    import { TaskService } from '$lib/services/TaskService.js';
    import Editor from '$lib/components/Editor.svelte';
    import Terminal from '$lib/components/Terminal.svelte';

    let isSaving = $state(false);
    let saveMsg  = $state('');
    let autoSave = $state(false);
    let autoSaveTimer;

    async function saveFile() {
        const path = ide.activeTab;
        if (!path) return;
        const tab = ide.openTabs.find(t => t.path === path);
        if (!tab || !tab.isDirty) return;
        isSaving = true;
        try {
            const r = await uplink.post('/api/fs/write', { path, content: tab.content ?? '' });
            if (r.ok) { ide.markSaved(path); saveMsg = '✓ COMMITTED'; }
        } catch(e) {
            saveMsg = '!! FAILED';
        } finally {
            isSaving = false;
            setTimeout(() => saveMsg = '', 2000);
        }
    }

    $effect(() => {
        const path = ide.activeTab;
        if (!autoSave || !path) return;
        const tab = ide.openTabs.find(t => t.path === path);
        if (tab?.isDirty && !isSaving) {
            clearTimeout(autoSaveTimer);
            autoSaveTimer = setTimeout(saveFile, 1000);
        }
    });

    function handleKeydown(e) {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
            e.preventDefault();
            saveFile();
        }
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="ide-shell">
    <div class="tab-bar">
        {#each ide.openTabs as tab (tab.path)}
            <button 
                type="button"
                class="ide-tab" 
                role="tab" 
                aria-selected={ide.activeTab === tab.path}
                class:tab-active={ide.activeTab === tab.path}
                onclick={() => ide.activeTab = tab.path}>
                <span class="tab-name">{tab.path.split('/').at(-1)}</span>
                {#if tab.isDirty}<span class="tab-dirty">●</span>{/if}
                <span class="tab-close" role="button" tabindex="0" aria-label="Close tab" 
                      onclick={(e) => { e.stopPropagation(); ide.closeTab(tab.path); }}
                      onkeydown={(e) => { if(e.key === "Enter") { e.stopPropagation(); ide.closeTab(tab.path); } }}>×</span>
            </button>
        {/each}
        <div class="tab-controls">
            {#if ide.activeTab}
                <span class="save-status">{saveMsg}</span>
                <button class="save-btn" onclick={saveFile}>COMMIT</button>
            {/if}
        </div>
    </div>
    <div class="editor-pane">
        {#if ide.activeTab}<Editor path={ide.activeTab} />
        {:else}
            <div class="ide-welcome">
                <div class="iw-glyph">◈</div>
                <div class="iw-title">AWAITING_INPUT</div>
            </div>
        {/if}
    </div>
    <div class="ide-term-strip"><Terminal /></div>
</div>

<style>
    .ide-shell { display:flex; flex-direction:column; height:100%; width:100%; }
    .tab-bar { flex:none; display:flex; height:34px; background:#010105; border-bottom:1px solid rgba(0,255,255,.1); overflow-x:auto; }
    .ide-tab { background: none; border: none; border-right: 1px solid rgba(0,255,255,.05); padding:0 12px; cursor:pointer; display:flex; align-items:center; gap:8px; font-size:9px; color:#64748b; font-family: inherit; transition: all 0.15s; }
    .tab-active { color:#00ffff; background:rgba(0,255,255,.03); border-bottom:2px solid #00ffff !important; }
    .tab-name { white-space: nowrap; }
    .tab-dirty { color:#fb923c; font-size:8px; }
    .tab-close { opacity:.4; font-size:14px; line-height:1; padding: 2px 5px; }
    .tab-close:hover { opacity:1; color:#f87171; }
    .tab-controls { flex:1; display:flex; align-items:center; justify-content:flex-end; padding:0 8px; gap:8px; }
    .save-status { font-size:8px; color:#34d399; }
    .save-btn { background:rgba(0,255,255,.1); border:1px solid rgba(0,255,255,.3); color:#00ffff; font-size:8px; padding:2px 10px; cursor:pointer; font-family:'JetBrains Mono',monospace; }
    .editor-pane { flex:1; background:#000; overflow:hidden; display:flex; align-items:center; justify-content:center; }
    .ide-term-strip { flex:none; height:180px; border-top:1px solid rgba(0,255,255,.05); overflow:hidden; display:flex; flex-direction:column; }
    .ide-welcome { display:flex; flex-direction:column; align-items:center; opacity:0.2; }
    .iw-glyph { font-size:48px; color:#00ffff; margin-bottom:10px; }
    .iw-title { font-size:12px; letter-spacing:0.2em; font-weight:bold; }
</style>
