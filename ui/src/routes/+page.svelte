<script>
// ═══════════════════════════════════════════════════════════════════════
//  NEXUS SINGULARITY — Master SPA v6.2.0
//  AUDIT FIXES:
//    ✓ File open: path used AS-IS (relative) — no /home/... prefix
//    ✓ NavHeader always mounted at top
//    ✓ GhostBlade always in DOM (layout.ghostVisible controls render)
//    ✓ StatusBar always at bottom
//    ✓ Terminal embedded in WORKSPACE + NEXUS_OS panels
//    ✓ store: wsState.ready, fileTreeTrigger.value, ide.openTabs[]
//    ✓ Auto-save 1s debounce via $effect
//    ✓ Ctrl+S manual save
// ═══════════════════════════════════════════════════════════════════════
import { ide, layout, fileTreeTrigger }  from '$lib/core/store.svelte.js';
import { pushTermLine, WS }               from '$lib/core/NexusWS.js';
import { fly }                            from 'svelte/transition';
import { onMount }                        from 'svelte';

import NavHeader   from '$lib/components/NavHeader.svelte';
import GhostBlade  from '$lib/components/GhostBlade.svelte';
import Sidebar     from '$lib/components/Sidebar.svelte';
import Editor      from '$lib/components/Editor.svelte';
import Terminal    from '$lib/components/Terminal.svelte';
import StatusBar   from '$lib/components/StatusBar.svelte';
import VitalsView  from '$lib/components/VitalsView.svelte';
import MemoryVault from '$lib/components/MemoryVault.svelte';
import SecOps      from '$lib/components/SecOps.svelte';
import AgentLab    from '$lib/components/AgentLab.svelte';

// ── NAV ────────────────────────────────────────────────────────────────
let activeNav = $state('NEXUS_OS');

// ── OPEN FILE ──────────────────────────────────────────────────────────
// CRITICAL FIX: path arrives from sidebar as relative e.g. "src/main.rs"
// We pass it directly — the Axum sandbox handler resolves from workspace root.
// Do NOT prepend anything. The old bug: `/home/donball197/..././${path}`
let openInput  = $state('');
let openError  = $state('');

async function openFile(path) {
    if (!path?.trim()) return;
    openError = '';

    // Already open? Just switch focus.
    if (ide.openTabs.some(t => t.path === path)) {
        ide.activeTab = path;
        activeNav = 'IDE';
        return;
    }

    try {
        const r = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
        if (!r.ok) {
            openError = `Cannot open: ${path} (HTTP ${r.status})`;
            pushTermLine(`!! [IDE] ${openError}`, 'text-rose-400');
            return;
        }
        const data = await r.json();
        ide.mountFile(path, data.content ?? '');
        activeNav = 'IDE';
        pushTermLine(`>> [IDE] Opened: ${path}`, 'text-emerald-400');
    } catch(e) {
        openError = e.message;
        pushTermLine(`!! [IDE] Open failed: ${e.message}`, 'text-rose-400');
    }
}

async function submitOpenInput(e) {
    e.preventDefault();
    const p = openInput.trim();
    if (p) { await openFile(p); openInput = ''; }
}

// ── CREATE FILE ────────────────────────────────────────────────────────
async function createFile(path) {
    if (!path) return;
    try {
        const r = await fetch('/api/fs/write', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path, content: `// ${path}\n// Created by NEXUS IDE — ${new Date().toISOString()}\n\n` })
        });
        if (r.ok) {
            pushTermLine(`>> [IDE] Created: ${path}`, 'text-emerald-400');
            fileTreeTrigger.value++;
            await openFile(path);
        } else {
            pushTermLine(`!! [IDE] Create failed: HTTP ${r.status}`, 'text-rose-400');
        }
    } catch(e) {
        pushTermLine(`!! [IDE] ${e.message}`, 'text-rose-400');
    }
}

// ── SAVE ───────────────────────────────────────────────────────────────
let isSaving     = $state(false);
let saveMsg      = $state('');
let autoSave     = $state(false);
let autoSaveTimer;

async function saveFile() {
    const path = ide.activeTab;
    if (!path) return;
    const tab = ide.openTabs.find(t => t.path === path);
    if (!tab || !tab.isDirty) return;

    isSaving = true;
    try {
        const r = await fetch('/api/fs/write', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path, content: tab.content ?? '' })
        });
        if (r.ok) {
            ide.markSaved(path);
            saveMsg = '✓ SAVED';
            pushTermLine(`>> [IDE] Saved: ${path}`, 'text-emerald-400');
        } else {
            saveMsg = `!! SAVE FAILED ${r.status}`;
            pushTermLine(`!! [IDE] Save failed: ${r.status}`, 'text-rose-400');
        }
    } catch(e) {
        saveMsg = `!! ${e.message}`;
    } finally {
        isSaving = false;
        setTimeout(() => saveMsg = '', 2500);
    }
}

// 1-second debounce auto-save
$effect(() => {
    const path = ide.activeTab;
    if (!autoSave || !path) return;
    const tab = ide.openTabs.find(t => t.path === path);
    if (tab?.isDirty && !isSaving) {
        clearTimeout(autoSaveTimer);
        autoSaveTimer = setTimeout(saveFile, 1000);
    }
});

// ── GLOBAL CTRL+S ──────────────────────────────────────────────────────
function handleGlobalKey(e) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        if (activeNav === 'IDE') {
            clearTimeout(autoSaveTimer);
            saveFile();
        }
    }
    // Ctrl+` → toggle terminal focus (future)
    if ((e.ctrlKey || e.metaKey) && e.key === '`') {
        e.preventDefault();
    }
}

// ── WORKSPACE QUICK ACTIONS ────────────────────────────────────────────
async function runDirective(directive) {
    pushTermLine(`>> [WORKSPACE] ${directive}`, 'text-cyan-400');
    try {
        const r = await fetch('/api/task', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ directive })
        });
        if (r.ok) pushTermLine('>> Task queued — watch terminal below.', 'text-emerald-400');
        else       pushTermLine(`!! HTTP ${r.status}`, 'text-rose-400');
    } catch(e) {
        pushTermLine(`!! ${e.message}`, 'text-rose-400');
    }
}

// ── LAZARUS / HISTORY ──────────────────────────────────────────────────
let snapshots    = $state([]);
let snapsLoading = $state(false);
let snapMsg      = $state('');

async function loadSnaps() {
    snapsLoading = true;
    snapMsg = '';
    try {
        const r = await fetch('/api/lazarus/snapshots');
        if (r.ok) snapshots = [...(await r.json())].reverse();
        else snapshots = [];
    } catch { snapshots = []; }
    finally { snapsLoading = false; }
}

async function restoreSnap(hash) {
    snapMsg = `Queuing restore for ${hash.slice(0,8)}...`;
    try {
        await fetch('/api/task', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ directive: `restore snapshot ${hash}` })
        });
        snapMsg = `✓ Restore queued for ${hash.slice(0,8)} — check terminal.`;
    } catch(e) {
        snapMsg = `!! ${e.message}`;
    }
    setTimeout(() => snapMsg = '', 5000);
}

$effect(() => { if (activeNav === 'HISTORY') loadSnaps(); });

// ── BOOT LOG ───────────────────────────────────────────────────────────
onMount(() => {
    pushTermLine('>> [NEXUS] Singularity OS v4.2.3 — ARM64 CROSTINI', 'text-cyan-400');
    pushTermLine('>> [NEXUS] All systems nominal. WebSocket connecting...', 'text-gray-400');
});
</script>

<svelte:window onkeydown={handleGlobalKey} />

<div class="nexus-root">

    <NavHeader bind:activeNav />

    <div class="body-row">

        {#if layout.explorerVisible}
            <aside class="sidebar-col" transition:fly={{ x: -224, duration: 160, opacity: 1 }}>
                <div class="open-box">
                    <div class="open-label">OPEN FILE</div>
                    <form onsubmit={submitOpenInput} class="open-row">
                        <input
                            bind:value={openInput}
                            placeholder="src/main.rs"
                            class="open-inp"
                            autocomplete="off"
                            spellcheck="false"
                        />
                        <button type="submit" class="open-go" title="Open">↵</button>
                    </form>
                    {#if openError}
                        <div class="open-err">{openError}</div>
                    {/if}
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
                            <div class="os-hero-sub">v4.2.3 // ARM64 CROSTINI // LENOVO DUET</div>
                        </div>
                    </div>
                    <div class="os-grid">
                        <div class="os-card os-card--main">
                            <div class="os-card-hdr">SYSTEM_ARCHITECTURE</div>
                            <div class="os-node-grid">
                                {#each [
                                    ['AXUM_SERVER',     '#22d3ee'],
                                    ['NEURAL_AGENT',    '#34d399'],
                                    ['ORCHESTRATOR',    '#a78bfa'],
                                    ['ARBITER',         '#f87171'],
                                    ['TASK_MANAGER',    '#fbbf24'],
                                    ['MEMORY_PALACE',   '#a78bfa'],
                                    ['SCHEDULER',       '#22d3ee'],
                                    ['LAZARUS_VAULT',   '#fb923c'],
                                    ['SELF_HEALER',     '#34d399'],
                                    ['SANDBOX',         '#f87171'],
                                    ['THERMAL_GOV',     '#fb923c'],
                                    ['LITERT_BRIDGE',   '#fbbf24'],
                                ] as [sys, c], i}
                                    <div class="os-node" style="animation-delay:{i*60}ms;border-color:{c}22">
                                        <span class="os-node-dot" style="background:{c};box-shadow:0 0 5px {c}"></span>
                                        <span class="os-node-lbl" style="color:{c}bb">{sys}</span>
                                    </div>
                                {/each}
                            </div>
                        </div>

                        <div class="os-card">
                            <div class="os-card-hdr">RUNTIME_STACK</div>
                            {#each [
                                ['PRIMARY_MODEL',  'GEMMA 4 31B-IT',        '#fbbf24'],
                                ['BACKEND',        'RUST / AXUM 0.7',        '#fb923c'],
                                ['FRONTEND',       'SVELTE 5 / VITE 5',      '#f97316'],
                                ['DATABASE',       'SQLITE WAL',             '#22d3ee'],
                                ['ALLOCATOR',      'MIMALLOC',               '#34d399'],
                                ['FFI_LAYER',      'ZIG 0.13',               '#34d399'],
                                ['ARCH',           'AARCH64 LINUX',          '#a78bfa'],
                                ['WEBSOCKET',      '/ws/nexus',              '#818cf8'],
                            ] as [k, v, c]}
                                <div class="os-kv">
                                    <span class="os-k">{k}</span>
                                    <span class="os-v" style="color:{c}">{v}</span>
                                </div>
                            {/each}
                        </div>
                    </div>

                    <div class="os-quick-row">
                        {#each [
                            { label:'OPEN IDE',      color:'#22d3ee', action:()=>activeNav='IDE'        },
                            { label:'WORKSPACE',     color:'#34d399', action:()=>activeNav='WORKSPACE'  },
                            { label:'VITALS',        color:'#fb923c', action:()=>activeNav='VITALS'     },
                            { label:'MEMORY PALACE', color:'#a78bfa', action:()=>activeNav='MEMORY'     },
                        ] as btn}
                            <button class="os-quick-btn" style="--bc:{btn.color}" onclick={btn.action}>
                                {btn.label} ›
                            </button>
                        {/each}
                    </div>

                    <div class="os-term-wrap">
                        <div class="panel-section-hdr" style="color:#22d3ee">SYSTEM_OUTPUT</div>
                        <Terminal />
                    </div>
                </div>

            {:else if activeNav === 'IDE'}
                <div class="ide-shell">
                    <div class="tab-bar">
                        {#each ide.openTabs as tab (tab.path)}
                            <button
                                class="ide-tab"
                                class:tab-active={ide.activeTab === tab.path}
                                onclick={() => ide.activeTab = tab.path}
                                title={tab.path}
                            >
                                <span class="tab-name">{tab.path.split('/').at(-1)}</span>
                                {#if tab.isDirty}
                                    <span class="tab-dirty" title="Unsaved changes">●</span>
                                {/if}
                                <span
                                    class="tab-close"
                                    role="button"
                                    tabindex="-1"
                                    onclick={(e) => { e.stopPropagation(); ide.closeTab(tab.path); }}
                                    onkeydown={(e) => e.key === 'Enter' && ide.closeTab(tab.path)}
                                >×</span>
                            </button>
                        {/each}

                        {#if ide.openTabs.length === 0}
                            <div class="tab-empty-hint">← open a file from the sidebar</div>
                        {/if}

                        <div class="tab-controls">
                            {#if ide.activeTab}
                                <label class="autosave-toggle" title="Toggle auto-save (1s debounce)">
                                    <input type="checkbox" bind:checked={autoSave} />
                                    <span class="as-track"></span>
                                    <span class="as-label">AUTO</span>
                                </label>
                                {#if isSaving}
                                    <span class="save-status saving">⟳ SAVING…</span>
                                {:else if saveMsg}
                                    <span class="save-status" class:save-ok={saveMsg.startsWith('✓')} class:save-err={saveMsg.startsWith('!!')}>
                                        {saveMsg}
                                    </span>
                                {/if}
                                <button class="save-btn" onclick={saveFile} disabled={isSaving} title="Save (Ctrl+S)">
                                    SAVE
                                </button>
                            {/if}
                        </div>
                    </div>

                    <div class="editor-pane">
                        {#if ide.activeTab}
                            <Editor path={ide.activeTab} />
                        {:else}
                            <div class="ide-welcome">
                                <div class="iw-glyph">◈</div>
                                <div class="iw-title">NEXUS IDE</div>
                                <div class="iw-hints">
                                    <span>← Type a path in OPEN FILE to load any file</span>
                                    <span>← Click a file in the tree</span>
                                    <span>Ctrl+S · save &nbsp;│&nbsp; Tab · 4 spaces &nbsp;│&nbsp; Ctrl+/ · comment</span>
                                </div>
                            </div>
                        {/if}
                    </div>

                    <div class="ide-term-strip">
                        <Terminal />
                    </div>
                </div>

            {:else if activeNav === 'WORKSPACE'}
                <div class="panel-scroll ws-panel">
                    <div class="ws-hero">
                        <span class="ws-hero-title">WORKSPACE_DNA_MAP</span>
                        <span class="ws-hero-sub">ARM64 BUILD PIPELINE // NEXUS SINGULARITY</span>
                    </div>

                    <div class="ws-grid">
                        <div class="ws-group" style="--gc:#22d3ee">
                            <div class="ws-group-title">BUILD_PIPELINE</div>
                            {#each [
                                ['CARGO CHECK',    'run cargo check and report every error with file and line numbers'],
                                ['CARGO CLIPPY',   'run cargo clippy and list all warnings with suggested fixes'],
                                ['NPM BUILD',      'run npm run build inside the ui directory and report result'],
                                ['RELEASE BUILD',  'cargo build --release --target aarch64-unknown-linux-gnu and show final binary size in MB'],
                            ] as [l, d]}
                                <button class="ws-btn" onclick={() => runDirective(d)}>▶ {l}</button>
                            {/each}
                        </div>

                        <div class="ws-group" style="--gc:#a78bfa">
                            <div class="ws-group-title">NEURAL_OPS</div>
                            {#each [
                                ['SECURITY AUDIT', 'audit all rust source files for unsafe blocks security vulnerabilities and hardcoded secrets'],
                                ['SCAN MARKERS',   'search all source files for TODO FIXME HACK SUTURE and UNSAFE markers with file locations'],
                                ['ORPHAN SCAN',    'list all rust files not declared in lib.rs module tree'],
                                ['DEAD CODE',      'run cargo check with dead code warnings and list unused functions'],
                            ] as [l, d]}
                                <button class="ws-btn" onclick={() => runDirective(d)}>▶ {l}</button>
                            {/each}
                        </div>

                        <div class="ws-group" style="--gc:#fb923c">
                            <div class="ws-group-title">LAZARUS_OPS</div>
                            {#each [
                                ['SNAPSHOT MAIN',  'take a lazarus snapshot of src/main.rs tagged manual-checkpoint'],
                                ['SNAPSHOT ALL',   'take lazarus snapshots of all modified rust source files'],
                                ['LIST VAULT',     'list all lazarus vault snapshots showing path hash and timestamp'],
                                ['VIEW HISTORY →', null],
                            ] as [l, d]}
                                <button class="ws-btn" onclick={() => d ? runDirective(d) : activeNav='HISTORY'}>▶ {l}</button>
                            {/each}
                        </div>
                    </div>

                    <div class="ws-term">
                        <div class="panel-section-hdr" style="color:#34d399">NEURAL_OUTPUT</div>
                        <Terminal />
                    </div>
                </div>

            {:else if activeNav === 'VITALS'}
                <div class="fill-panel"><VitalsView /></div>

            {:else if activeNav === 'MEMORY'}
                <div class="fill-panel"><MemoryVault /></div>

            {:else if activeNav === 'SECOPS'}
                <div class="fill-panel"><SecOps /></div>

            {:else if activeNav === 'AGENT-LAB'}
                <div class="fill-panel"><AgentLab /></div>

            {:else if activeNav === 'HISTORY'}
                <div class="panel-scroll history-panel">
                    <div class="hist-hdr">
                        <div>
                            <div class="hist-title">LAZARUS_VAULT</div>
                            <div class="hist-sub">TEMPORAL LEDGER // FILE SNAPSHOTS // TIME-TRAVEL RESTORE</div>
                        </div>
                        <button class="hist-refresh" onclick={loadSnaps}>⟳ REFRESH</button>
                    </div>
                    {#if snapMsg}
                        <div class="snap-msg">{snapMsg}</div>
                    {/if}
                    {#if snapsLoading}
                        <div class="hist-loading">
                            <div class="loading-spin"></div>
                            SCANNING VAULT...
                        </div>
                    {:else if snapshots.length === 0}
                        <div class="hist-empty">
                            <div style="font-size:36px;opacity:.15;margin-bottom:12px;">◎</div>
                            NO SNAPSHOTS FOUND<br>
                            <span style="font-size:10px;opacity:.5;margin-top:6px;display:block">
                                Run WORKSPACE → SNAPSHOT MAIN to begin versioning
                            </span>
                        </div>
                    {:else}
                        <div class="snap-list">
                            {#each snapshots as snap}
                                <div class="snap-row">
                                    <div class="snap-info">
                                        <div class="snap-path">{snap.path}</div>
                                        <div class="snap-meta">
                                            <span class="snap-hash">{snap.hash?.slice(0,12) ?? '—'}</span>
                                            <span class="snap-ts">{snap.timestamp}</span>
                                        </div>
                                    </div>
                                    <div class="snap-actions">
                                        <button class="snap-open" onclick={() => openFile(snap.path)}>OPEN</button>
                                        <button class="snap-restore" onclick={() => restoreSnap(snap.hash)}>RESTORE</button>
                                    </div>
                                </div>
                            {/each}
                        </div>
                    {/if}
                </div>

            {:else}
                <div class="panel-placeholder">[{activeNav}_MODULE_OFFLINE]</div>
            {/if}

        </main>

        <GhostBlade />

    </div>

    <StatusBar />

</div>

<style>
/* ── Reset & root ─────────────────────────────────────────────────── */
:global(html, body) {
    margin: 0; padding: 0; height: 100vh;
    background: #000; overflow: hidden;
}
:global(*) { box-sizing: border-box; }
:global(::-webkit-scrollbar)       { width: 4px; height: 4px; }
:global(::-webkit-scrollbar-track) { background: #000; }
:global(::-webkit-scrollbar-thumb) { background: #1e3a4a; border-radius: 2px; }

.nexus-root {
    position: fixed; inset: 0;
    display: flex; flex-direction: column;
    font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
    background: #000; color: #e2e8f0;
}

/* ── Layout ───────────────────────────────────────────────────────── */
.body-row  { flex: 1; display: flex; overflow: hidden; min-height: 0; }
.main-col  { flex: 1; display: flex; flex-direction: column; overflow: hidden; min-width: 0; }
.fill-panel { flex: 1; overflow: hidden; display: flex; flex-direction: column; }
.panel-scroll {
    flex: 1; overflow-y: auto; padding: 18px;
}

/* ── Sidebar ──────────────────────────────────────────────────────── */
.sidebar-col {
    width: 224px; flex: none;
    display: flex; flex-direction: column;
    border-right: 1px solid rgba(0,255,255,.07);
    background: #020208; overflow: hidden;
}
.open-box  { flex: none; padding: 8px; border-bottom: 1px solid rgba(0,255,255,.05); }
.open-label { font-size: 7px; font-weight: 900; letter-spacing: .15em; color: rgba(0,255,255,.25); margin-bottom: 4px; }
.open-row  { display: flex; gap: 3px; }
.open-inp  {
    flex: 1; background: #000; border: 1px solid rgba(0,255,255,.12);
    color: #22d3ee; font-size: 10px; padding: 4px 6px; outline: none;
    font-family: inherit; transition: border-color .15s;
}
.open-inp:focus { border-color: rgba(0,255,255,.4); }
.open-inp::placeholder { color: rgba(0,255,255,.2); }
.open-go  {
    background: rgba(34,211,238,.1); border: 1px solid rgba(0,255,255,.2);
    color: #22d3ee; font-size: 15px; padding: 0 8px; cursor: pointer; font-family: inherit;
    transition: background .12s;
}
.open-go:hover { background: rgba(34,211,238,.22); }
.open-err { font-size: 8px; color: #f87171; margin-top: 3px; line-height: 1.4; }

/* ── Nexus OS panel ───────────────────────────────────────────────── */
.nexus-os-panel { display: flex; flex-direction: column; gap: 14px; }
.os-hero {
    display: flex; align-items: center; gap: 16px;
    padding: 14px; border: 1px solid rgba(0,255,255,.1);
    background: linear-gradient(135deg, rgba(0,255,255,.03) 0%, transparent 100%);
}
.os-hero-glyph {
    font-size: 40px; color: rgba(0,255,255,.3);
    animation: glyph-pulse 4s ease-in-out infinite;
}
@keyframes glyph-pulse { 0%,100%{opacity:.3;} 50%{opacity:.7; text-shadow: 0 0 30px rgba(0,255,255,.4);} }
.os-hero-title { font-size: 16px; font-weight: 900; letter-spacing: .2em; color: #00ffff; text-shadow: 0 0 20px rgba(0,255,255,.4); }
.os-hero-sub   { font-size: 9px; color: rgba(0,255,255,.3); margin-top: 3px; letter-spacing: .1em; }

.os-grid { display: grid; grid-template-columns: 1fr 260px; gap: 10px; }
.os-card {
    border: 1px solid rgba(0,255,255,.07); padding: 12px;
    background: rgba(0,255,255,.01);
}
.os-card-hdr { font-size: 8px; font-weight: 900; letter-spacing: .15em; color: rgba(0,255,255,.3); margin-bottom: 10px; }
.os-node-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 4px; }
.os-node {
    display: flex; align-items: center; gap: 6px;
    padding: 5px 8px; border: 1px solid;
    font-size: 8px; letter-spacing: .04em;
    animation: node-in .3s ease both;
}
@keyframes node-in { from{opacity:0;transform:translateY(4px);} to{opacity:1;transform:none;} }
.os-node-dot  { width: 5px; height: 5px; border-radius: 50%; flex: none; animation: dot-blink 2.5s infinite; }
@keyframes dot-blink { 0%,100%{opacity:1;} 50%{opacity:.25;} }
.os-node-lbl  { font-size: 8px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.os-kv { display: flex; justify-content: space-between; align-items: center; padding: 5px 0; border-bottom: 1px solid rgba(0,255,255,.04); font-size: 9px; }
.os-k  { color: rgba(255,255,255,.22); }
.os-v  { font-weight: 700; }

.os-quick-row { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; }
.os-quick-btn {
    padding: 8px; background: rgba(0,0,0,.4); border: 1px solid var(--bc);
    color: var(--bc); font-size: 9px; font-weight: 900; letter-spacing: .06em;
    cursor: pointer; font-family: inherit; transition: all .12s; text-align: center;
}
.os-quick-btn:hover { background: color-mix(in srgb, var(--bc) 15%, transparent); box-shadow: 0 0 10px color-mix(in srgb, var(--bc) 30%, transparent); }

.os-term-wrap { border: 1px solid rgba(0,255,255,.06); height: 220px; flex: none; }

/* ── IDE ──────────────────────────────────────────────────────────── */
.ide-shell { flex: 1; display: flex; flex-direction: column; overflow: hidden; }

.tab-bar {
    flex: none; display: flex; align-items: stretch; height: 34px;
    background: #000; border-bottom: 1px solid rgba(0,255,255,.06);
    overflow-x: auto; overflow-y: hidden;
}
.tab-bar::-webkit-scrollbar { height: 1px; }

.ide-tab {
    display: flex; align-items: center; gap: 5px;
    padding: 0 10px; min-width: 90px; max-width: 180px;
    border: none; border-right: 1px solid rgba(0,255,255,.04);
    background: transparent; color: rgba(255,255,255,.2);
    font-size: 9px; font-family: inherit; cursor: pointer;
    white-space: nowrap; transition: all .1s; flex: none;
}
.ide-tab:hover { color: rgba(0,255,255,.5); background: rgba(0,255,255,.03); }
.tab-active {
    background: #0a0a18; color: #22d3ee;
    border-bottom: 2px solid #22d3ee;
    text-shadow: 0 0 8px rgba(0,255,255,.4);
}
.tab-name  { flex: 1; overflow: hidden; text-overflow: ellipsis; }
.tab-dirty { color: #fb923c; font-size: 10px; flex: none; }
.tab-close {
    opacity: .3; flex: none; font-size: 14px; padding: 0 2px;
    transition: all .1s; line-height: 1; border-radius: 2px;
}
.tab-close:hover { opacity: 1; background: rgba(248,113,113,.2); color: #f87171; }
.tab-empty-hint { padding: 0 14px; font-size: 9px; color: rgba(0,255,255,.12); align-self: center; font-style: italic; }

.tab-controls {
    margin-left: auto; display: flex; align-items: center; gap: 8px;
    padding: 0 12px; flex: none; border-left: 1px solid rgba(0,255,255,.05);
}
.autosave-toggle {
    display: flex; align-items: center; gap: 5px; cursor: pointer;
}
.autosave-toggle input { display: none; }
.as-track {
    position: relative; width: 26px; height: 13px;
    background: rgba(0,255,255,.1); border: 1px solid rgba(0,255,255,.2);
    border-radius: 7px; transition: .2s;
}
.as-track::after {
    content: ''; position: absolute; top: 1px; left: 1px;
    width: 9px; height: 9px; background: rgba(0,255,255,.4);
    border-radius: 50%; transition: .2s;
}
.autosave-toggle input:checked ~ .as-track { background: rgba(34,211,238,.2); border-color: #22d3ee; }
.autosave-toggle input:checked ~ .as-track::after { transform: translateX(13px); background: #22d3ee; box-shadow: 0 0 5px #22d3ee; }
.as-label { font-size: 8px; font-weight: 900; color: rgba(0,255,255,.3); letter-spacing: .08em; }
.autosave-toggle input:checked ~ .as-label { color: #22d3ee; }

.save-status { font-size: 9px; font-weight: 700; white-space: nowrap; color: rgba(255,255,255,.3); }
.save-status.saving { color: #fbbf24; }
.save-status.save-ok { color: #34d399; }
.save-status.save-err { color: #f87171; }
.save-btn {
    background: rgba(34,211,238,.08); border: 1px solid rgba(0,255,255,.2);
    color: #22d3ee; font-size: 8px; font-weight: 900; padding: 2px 9px; cursor: pointer;
    font-family: inherit; transition: background .12s; letter-spacing: .08em;
}
.save-btn:hover:not(:disabled) { background: rgba(34,211,238,.2); }
.save-btn:disabled { opacity: .4; cursor: not-allowed; }

.editor-pane { flex: 1; overflow: hidden; }
.ide-term-strip { flex: none; height: 180px; border-top: 1px solid rgba(0,255,255,.06); }

.ide-welcome {
    height: 100%; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 14px;
    background: #010105;
}
.iw-glyph { font-size: 52px; color: rgba(0,255,255,.06); animation: iw-pulse 4s infinite; }
@keyframes iw-pulse { 0%,100%{opacity:.2;} 50%{opacity:.5; text-shadow: 0 0 40px rgba(0,255,255,.2);} }
.iw-title  { font-size: 16px; font-weight: 900; letter-spacing: .3em; color: rgba(0,255,255,.12); }
.iw-hints  { display: flex; flex-direction: column; gap: 5px; text-align: center; }
.iw-hints span { font-size: 9px; color: rgba(0,255,255,.1); }

/* ── Workspace ────────────────────────────────────────────────────── */
.ws-panel { display: flex; flex-direction: column; gap: 14px; }
.ws-hero  { padding: 12px 0 4px; border-bottom: 1px solid rgba(52,211,153,.1); margin-bottom: 2px; }
.ws-hero-title { font-size: 13px; font-weight: 900; font-style: italic; letter-spacing: .08em; color: #34d399; text-shadow: 0 0 12px rgba(52,211,153,.3); }
.ws-hero-sub   { font-size: 8px; color: rgba(52,211,153,.3); margin-top: 3px; letter-spacing: .08em; display: block; }

.ws-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
.ws-group {
    padding: 12px; border: 1px solid rgba(255,255,255,.04);
    display: flex; flex-direction: column; gap: 5px;
    background: rgba(0,0,0,.3);
}
.ws-group-title {
    font-size: 8px; font-weight: 900; letter-spacing: .12em;
    text-decoration: underline; color: var(--gc); margin-bottom: 3px;
}
.ws-btn {
    width: 100%; text-align: left; padding: 7px 9px;
    border: 1px solid color-mix(in srgb, var(--gc) 25%, transparent);
    background: color-mix(in srgb, var(--gc) 4%, transparent);
    color: color-mix(in srgb, var(--gc) 70%, white);
    font-size: 8px; font-weight: 700; letter-spacing: .04em;
    font-family: inherit; cursor: pointer; transition: all .12s;
}
.ws-btn:hover { background: color-mix(in srgb, var(--gc) 12%, transparent); }

.ws-term { border: 1px solid rgba(0,255,255,.06); height: 220px; flex: none; }

/* ── Shared section header ────────────────────────────────────────── */
.panel-section-hdr {
    padding: 5px 10px; font-size: 8px; font-weight: 900; letter-spacing: .15em;
    background: rgba(0,0,0,.5); border-bottom: 1px solid rgba(255,255,255,.04);
}

/* ── History ──────────────────────────────────────────────────────── */
.history-panel { display: flex; flex-direction: column; gap: 12px; }
.hist-hdr  { display: flex; justify-content: space-between; align-items: flex-start; }
.hist-title { font-size: 12px; font-weight: 900; letter-spacing: .15em; color: #818cf8; }
.hist-sub   { font-size: 8px; color: rgba(129,140,248,.3); margin-top: 3px; }
.hist-refresh {
    background: rgba(129,140,248,.1); border: 1px solid rgba(129,140,248,.2);
    color: #818cf8; font-size: 9px; font-weight: 900; padding: 5px 12px;
    cursor: pointer; font-family: inherit; transition: background .12s; letter-spacing: .06em;
}
.hist-refresh:hover { background: rgba(129,140,248,.22); }
.snap-msg    { padding: 8px 12px; border: 1px solid rgba(129,140,248,.2); color: #818cf8; font-size: 9px; }
.hist-loading {
    display: flex; flex-direction: column; align-items: center; gap: 12px;
    padding: 48px; color: rgba(129,140,248,.3); font-size: 9px; letter-spacing: .15em;
}
.loading-spin {
    width: 28px; height: 28px; border-radius: 50%;
    border: 2px solid rgba(129,140,248,.1); border-top-color: #818cf8;
    animation: spin 1s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }
.hist-empty {
    padding: 60px; text-align: center; color: rgba(129,140,248,.25);
    font-size: 10px; letter-spacing: .15em; line-height: 2;
}
.snap-list { display: flex; flex-direction: column; gap: 4px; }
.snap-row {
    display: flex; align-items: center; gap: 12px;
    padding: 10px 12px; border: 1px solid rgba(129,140,248,.1);
    background: rgba(129,140,248,.02); transition: border-color .15s;
}
.snap-row:hover { border-color: rgba(129,140,248,.25); }
.snap-info { flex: 1; min-width: 0; }
.snap-path { font-size: 10px; color: #818cf8; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.snap-meta { display: flex; gap: 10px; margin-top: 3px; }
.snap-hash { font-size: 9px; color: rgba(129,140,248,.4); font-family: monospace; }
.snap-ts   { font-size: 9px; color: rgba(255,255,255,.2); }
.snap-actions { flex: none; display: flex; gap: 4px; }
.snap-open, .snap-restore {
    padding: 5px 10px; border: 1px solid rgba(129,140,248,.2);
    color: rgba(129,140,248,.6); background: transparent;
    font-size: 8px; font-weight: 900; letter-spacing: .1em;
    font-family: inherit; cursor: pointer; transition: all .12s;
}
.snap-open:hover    { background: rgba(34,211,238,.1);    color: #22d3ee;  border-color: rgba(34,211,238,.3); }
.snap-restore:hover { background: rgba(129,140,248,.12);  color: #818cf8;  border-color: rgba(129,140,248,.4); }

/* ── Placeholder ──────────────────────────────────────────────────── */
.panel-placeholder {
    flex: 1; display: flex; align-items: center; justify-content: center;
    font-size: 12px; font-weight: 900; letter-spacing: 4px; color: rgba(0,255,255,.06);
}
</style>
