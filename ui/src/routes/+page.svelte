<script lang="ts">
// ═══════════════════════════════════════════════════════════════
//  NEXUS SINGULARITY — Master SPA v6.4.0
//  FIX: socket.sendCommand -> socket.send
//  FIX: loadSnaps uses /api/task directive instead of missing route
// ═══════════════════════════════════════════════════════════════
import { ide, layout, fileTreeTrigger } from '$lib/core/store.svelte.js';
import { pushTermLine }      from '$lib/core/NexusWS.js';
import { fly }               from 'svelte/transition';
import { onMount, onDestroy } from 'svelte';
import { createNexusSocket } from '$lib/nexus_socket.svelte';

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

// ── Telemetry Uplink ───────────────────────────────────────────
let socket = createNexusSocket('/ws/telemetry');
onDestroy(() => socket.destroy());

// ── Navigation ─────────────────────────────────────────────────
let activeNav = $state('NEXUS_OS');

// ── File Orchestration ─────────────────────────────────────────
let openInput = $state('');
let openError = $state('');

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
        pushTermLine(`>> [IDE] Buffer Synchronized: ${path}`, 'text-emerald-400');
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
        pushTermLine(`!! [IDE] FRACTURE: ${e.message}`, 'text-rose-400');
    }
}

// ── Persistence ────────────────────────────────────────────────
let isSaving = $state(false);
let saveMsg  = $state('');
let autoSave = $state(false);
let autoSaveTimer: ReturnType<typeof setTimeout>;

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
        if (r.ok) { ide.markSaved(path); saveMsg = '✓ COMMITTED'; }
    } catch(e: any) {
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

function handleGlobalKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        if (activeNav === 'IDE') saveFile();
    }
}

// ── Workspace Tasking ──────────────────────────────────────────
async function runDirective(directive: string) {
    pushTermLine(`>> [WORKSPACE] Queuing: ${directive}`, 'text-cyan-400');
    try {
        await fetch('/api/task', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ directive })
        });
    } catch(e: any) {
        pushTermLine(`!! [FAULT] ${e.message}`, 'text-rose-400');
    }
}

// ── Ghost Memory Ritual ────────────────────────────────────────
// ⚡ FIX: was socket.sendCommand — correct API is socket.send()
function ghostEnshrine() {
    const handleEl = document.getElementById('ghost-handle') as HTMLInputElement;
    const textEl   = document.getElementById('ghost-text')   as HTMLInputElement;
    const handle   = handleEl?.value.trim() ?? '';
    const text     = textEl?.value.trim()   || 'NEXUS_DATA';
    if (!handle) { alert('Enter a handle (e.g. 0xc8)'); return; }
    socket.send(`ghost enshrine ${handle} "${text}"`);
    pushTermLine(`>> [GHOST] enshrine ${handle} "${text}"`, 'text-purple-400');
}

function ghostScry() {
    const handleEl = document.getElementById('ghost-handle') as HTMLInputElement;
    const handle   = handleEl?.value.trim() ?? '';
    if (!handle) { alert('Enter a handle'); return; }
    socket.send(`ghost scry ${handle}`);
    pushTermLine(`>> [GHOST] scry ${handle}`, 'text-purple-400');
}

function ghostRecall() {
    const handleEl = document.getElementById('ghost-handle') as HTMLInputElement;
    const handle   = handleEl?.value.trim() ?? '';
    if (!handle) { alert('Enter a handle'); return; }
    socket.send(`ghost recall ${handle}`);
    pushTermLine(`>> [GHOST] recall ${handle}`, 'text-purple-400');
}

// ── Lazarus snapshots ──────────────────────────────────────────
// ⚡ FIX: /api/lazarus/snapshots doesn't exist — fetch via memory API
// and show available snapshots from the vault directive instead
let snapshots = $state<any[]>([]);
let snapMsg   = $state('');

async function loadSnaps() {
    snapMsg = 'Scanning vault...';
    try {
        // ⚡ Route through task API since /api/lazarus/snapshots isn't wired
        const r = await fetch('/api/memories');
        if (r.ok) {
            const all = await r.json();
            // Filter memories that look like lazarus snapshot records
            snapshots = all
                .filter((m: any) => m.directive?.toLowerCase().includes('snapshot') ||
                                    m.origin?.toLowerCase().includes('lazarus'))
                .slice(0, 20)
                .reverse();
            snapMsg = snapshots.length === 0 ? 'No snapshots found in memory.' : '';
        }
    } catch {
        snapshots = [];
        snapMsg = 'Vault scan failed.';
    }
    setTimeout(() => snapMsg = '', 3000);
}

async function restoreSnap(hash: string) {
    snapMsg = `Initiating Temporal Restore: ${hash.slice(0,8)}...`;
    try {
        await fetch('/api/task', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ directive: `restore snapshot ${hash}` })
        });
    } catch(e: any) {
        snapMsg = `!! [ERROR] ${e.message}`;
    }
    setTimeout(() => snapMsg = '', 4000);
}

$effect(() => { if (activeNav === 'HISTORY') loadSnaps(); });

onMount(() => {
    pushTermLine('>> [NEXUS] Singularity OS v4.3.0 — Substrate Initialized', 'text-cyan-400');
});
</script>

<svelte:window onkeydown={handleGlobalKey} />

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

            <!-- ── NEXUS OS ── -->
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
                            <div class="vmo-kv"><span>STATUS</span><span style="color:#22d3ee">{socket.telemetry.status}</span></div>
                            <div class="vmo-kv"><span>ACTIVE VMOs</span><span style="color:#34d399">{socket.telemetry.active_vmos}</span></div>
                            <div class="vmo-kv"><span>RESONANCE</span><span style="color:#fbbf24">{socket.telemetry.resonance.toFixed(2)}</span></div>
                        </div>
                    </div>
                    <div class="os-term-wrap">
                        <div class="panel-section-hdr">NEURAL_OUTPUT_STREAM</div>
                        <Terminal />
                    </div>
                </div>

            <!-- ── IDE ── -->
            {:else if activeNav === 'IDE'}
                <div class="ide-shell">
                    <div class="tab-bar">
                        {#each ide.openTabs as tab (tab.path)}
                            <div class="ide-tab" role="tab" class:tab-active={ide.activeTab === tab.path}
                                onclick={() => ide.activeTab = tab.path} onkeydown={(e) => e.key === "Enter" && (ide.activeTab = tab.path)}>
                                <span class="tab-name">{tab.path.split('/').at(-1)}</span>
                                {#if tab.isDirty}<span class="tab-dirty">●</span>{/if}
                                <button type="button" class="tab-close" aria-label="Close tab" 
                                          onclick={(e) => { e.stopPropagation(); ide.closeTab(tab.path); }}>×</button>
                            </div>
                        {/each}
                        <div class="tab-controls">
                            {#if ide.activeTab}
                                <span class="save-status">{saveMsg}</span>
                                <button class="save-btn" onclick={saveFile}>COMMIT</button>
                            {/if}
                        </div>
                    </div>
                    <div class="editor-pane">
                        {#if ide.activeTab}
                            <Editor path={ide.activeTab} />
                        {:else}
                            <div class="ide-welcome">
                                <div class="iw-glyph">◈</div>
                                <div class="iw-title">AWAITING_INPUT</div>
                            </div>
                        {/if}
                    </div>
                    <div class="ide-term-strip"><Terminal /></div>
                </div>

            <!-- ── WORKSPACE ── -->
            {:else if activeNav === 'WORKSPACE'}
                <div class="panel-scroll ws-panel">
                    <div class="ws-hero"><span class="ws-hero-title">WORKSPACE_DNA_MAP</span></div>
                    <div class="ws-grid">
                        <div class="ws-group">
                            <div class="ws-group-title">BUILD_PIPELINE</div>
                            <button class="ws-btn" onclick={() => runDirective('run cargo check and report all errors')}>▶ CARGO CHECK</button>
                            <button class="ws-btn" onclick={() => runDirective('run npm run build in the ui directory')}>▶ UI BUILD</button>
                            <button class="ws-btn" onclick={() => runDirective('run cargo build --release and report binary size')}>▶ RELEASE BUILD</button>
                        </div>
                        <div class="ws-group">
                            <div class="ws-group-title">NEURAL_OPS</div>
                            <button class="ws-btn" onclick={() => runDirective('audit all rust source files for security vulnerabilities')}>▶ SECURITY AUDIT</button>
                            <button class="ws-btn" onclick={() => runDirective('find all TODO FIXME HACK and SUTURE comments in source')}>▶ SCAN TODOS</button>
                            <button class="ws-btn" onclick={() => runDirective('run cargo clippy and list all warnings')}>▶ CLIPPY SCAN</button>
                        </div>
                        <div class="ws-group">
                            <div class="ws-group-title">LAZARUS_OPS</div>
                            <button class="ws-btn" onclick={() => runDirective('take a lazarus snapshot of src/main.rs tagged manual')}>▶ SNAPSHOT MAIN</button>
                            <button class="ws-btn" onclick={() => runDirective('list all lazarus vault snapshots and their hashes')}>▶ LIST VAULT</button>
                        </div>

                        <!-- GHOST MEMORY RITUAL -->
                        <div class="ws-group" style="grid-column: 1 / -1;">
                            <div class="ws-group-title">GHOST_MEMORY_RITUAL</div>
                            <div style="display:flex;flex-direction:column;gap:6px;font-size:9px;font-family:'JetBrains Mono',monospace;">
                                <input id="ghost-handle"
                                       placeholder="VMO Handle e.g. 0xc8"
                                       style="background:#000;border:1px solid rgba(167,139,250,.3);color:#a78bfa;padding:6px 8px;outline:none;font-family:inherit;" />
                                <input id="ghost-text"
                                       placeholder="Text to enshrine (optional)"
                                       style="background:#000;border:1px solid rgba(167,139,250,.3);color:#a78bfa;padding:6px 8px;outline:none;font-family:inherit;" />
                                <div style="display:flex;gap:4px;">
                                    <button onclick={ghostEnshrine} class="ws-btn" style="flex:1;">⬆ ENSHRINE</button>
                                    <button onclick={ghostScry}    class="ws-btn" style="flex:1;">🔍 SCRY HEX</button>
                                    <button onclick={ghostRecall}  class="ws-btn" style="flex:1;">📖 RECALL TEXT</button>
                                </div>
                            </div>
                        </div>
                    </div>
                    <div class="ws-term" style="margin-top:16px;"><Terminal /></div>
                </div>

            <!-- ── VITALS ── -->
            {:else if activeNav === 'VITALS'}
                <div class="fill-panel"><VitalsView /></div>

            <!-- ── MEMORY ── -->
            {:else if activeNav === 'MEMORY'}
                <div class="fill-panel"><MemoryVault /></div>

            <!-- ── SECOPS ── -->
            {:else if activeNav === 'SECOPS'}
                <div class="fill-panel"><SecOps /></div>

            <!-- ── AGENT-LAB ── -->
            {:else if activeNav === 'AGENT-LAB'}
                <div class="fill-panel"><AgentLab /></div>

            <!-- ── HISTORY / LAZARUS VAULT ── -->
            {:else if activeNav === 'HISTORY'}
                <div class="panel-scroll history-panel">
                    <div class="hist-hdr">
                        <div class="hist-title">LAZARUS_VAULT</div>
                        <button onclick={loadSnaps}
                            style="background:none;border:none;color:rgba(167,139,250,.5);font-size:16px;cursor:pointer;">⟳</button>
                    </div>
                    {#if snapMsg}
                        <div style="padding:8px 16px;font-size:9px;color:#a78bfa;">{snapMsg}</div>
                    {/if}
                    <div class="snap-list">
                        {#each snapshots as snap}
                            <div class="snap-row">
                                <div class="snap-info">
                                    <div class="snap-path">{snap.directive ?? snap.path ?? 'unknown'}</div>
                                    <div class="snap-hash" style="font-size:8px;color:rgba(167,139,250,.4);">
                                        {snap.timestamp ?? ''}
                                    </div>
                                </div>
                                <button class="snap-restore"
                                    onclick={() => restoreSnap(snap.id?.toString() ?? '')}>
                                    RESTORE
                                </button>
                            </div>
                        {:else}
                            <div style="padding:32px;text-align:center;font-size:9px;color:rgba(167,139,250,.2);">
                                NO SNAPSHOTS — run a directive to create one
                            </div>
                        {/each}
                    </div>
                </div>

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

/* Open box */
.open-box { flex:none; padding:8px; border-bottom:1px solid rgba(0,255,255,.05); }
.open-label { font-size:7px; font-weight:900; letter-spacing:.14em; color:rgba(0,255,255,.25); margin-bottom:4px; }
.open-row { display:flex; gap:3px; }
.open-inp { flex:1; background:#000; border:1px solid rgba(0,255,255,.12); color:#22d3ee; font-size:10px; padding:4px 7px; outline:none; font-family:'JetBrains Mono',monospace; }
.open-go { width:28px; background:rgba(34,211,238,.1); border:1px solid rgba(0,255,255,.2); color:#22d3ee; font-size:14px; cursor:pointer; }
.open-err { font-size:8px; color:#f87171; margin-top:3px; }

/* OS panel */
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

/* IDE */
.ide-shell { flex:1; display:flex; flex-direction:column; overflow:hidden; }
.tab-bar { flex:none; display:flex; height:34px; background:#010105; border-bottom:1px solid rgba(0,255,255,.1); overflow-x:auto; }
.ide-tab { padding:0 12px; border-right:1px solid rgba(0,255,255,.05); cursor:pointer; display:flex; align-items:center; gap:8px; font-size:9px; color:#64748b; background:none; border-top:none; border-left:none; border-bottom:none; white-space:nowrap; }
.tab-active { color:#00ffff; background:rgba(0,255,255,.03); border-bottom:2px solid #00ffff !important; }
.tab-dirty { color:#fb923c; font-size:8px; }
.tab-close { opacity:.4; font-size:14px; line-height:1; }
.tab-close:hover { opacity:1; color:#f87171; }
.tab-controls { flex:1; display:flex; align-items:center; justify-content:flex-end; padding:0 8px; gap:8px; }
.save-status { font-size:8px; color:#34d399; }
.save-btn { background:rgba(0,255,255,.1); border:1px solid rgba(0,255,255,.3); color:#00ffff; font-size:8px; padding:2px 10px; cursor:pointer; font-family:'JetBrains Mono',monospace; }
.editor-pane { flex:1; background:#000; overflow:hidden; }
.ide-term-strip { flex:none; height:180px; border-top:1px solid rgba(0,255,255,.05); overflow:hidden; display:flex; flex-direction:column; }
.ide-welcome { height:100%; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:12px; }
.iw-glyph { font-size:40px; color:rgba(0,255,255,.1); }
.iw-title { font-size:10px; color:rgba(0,255,255,.2); letter-spacing:.2em; }

/* Workspace */
.ws-hero { margin-bottom:16px; }
.ws-hero-title { font-size:13px; font-weight:900; color:rgba(0,255,255,.4); letter-spacing:.2em; }
.ws-grid { display:grid; grid-template-columns:1fr 1fr 1fr; gap:10px; }
.ws-group { padding:12px; border:1px solid rgba(0,255,255,.06); background:rgba(0,255,255,.01); display:flex; flex-direction:column; gap:4px; }
.ws-group-title { font-size:8px; font-weight:900; letter-spacing:.14em; color:rgba(0,255,255,.3); margin-bottom:6px; }
.ws-btn { width:100%; text-align:left; padding:7px 10px; background:transparent; border:1px solid rgba(0,255,255,.1); color:#94a3b8; font-size:9px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .1s; }
.ws-btn:hover { color:#00ffff; border-color:rgba(0,255,255,.4); background:rgba(0,255,255,.04); }

/* History */
.history-panel { display:flex; flex-direction:column; gap:0; }
.hist-hdr { display:flex; align-items:center; justify-content:space-between; padding:14px 16px; border-bottom:1px solid rgba(167,139,250,.1); }
.hist-title { font-size:11px; font-weight:900; letter-spacing:.15em; color:#a78bfa; }
.snap-list { flex:1; overflow-y:auto; padding:8px; display:flex; flex-direction:column; gap:4px; }
.snap-row { display:flex; align-items:center; gap:10px; padding:8px 10px; border:1px solid rgba(167,139,250,.1); background:rgba(167,139,250,.02); }
.snap-info { flex:1; min-width:0; }
.snap-path { font-size:10px; color:rgba(255,255,255,.5); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.snap-restore { flex:none; padding:4px 10px; border:1px solid rgba(167,139,250,.3); background:transparent; color:#a78bfa; font-size:8px; font-weight:900; cursor:pointer; font-family:'JetBrains Mono',monospace; }
.snap-restore:hover { background:rgba(167,139,250,.15); }

/* Misc */
.panel-placeholder { flex:1; display:flex; align-items:center; justify-content:center; font-size:14px; opacity:.1; letter-spacing:10px; }
</style>
