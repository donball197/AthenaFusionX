#!/usr/bin/env bash
# CWD_CHECK
[[ "$PWD" != *"/nexus_singularity-dev"* ]] && echo "Error: Run from project root" && exit 1
# ============================================================
#  NEXUS SINGULARITY — FULL UPGRADE PATCH
#  Run from: ~/nexus_singularity-dev/
#  Writes every upgraded UI source file in place.
# ============================================================
set -e
cd "$(dirname "$0")" 2>/dev/null || true

echo ">> [NEXUS] Applying upgrade patch..."

# ─────────────────────────────────────────────────────────────
# 1. STORE.JS — adds pendingPatch + ghostLines stores
# ─────────────────────────────────────────────────────────────
cat > ui/src/lib/store.js << 'EOF'
import { writable } from 'svelte/store';

// ── Sector / Tab Management ─────────────────────────────────
export const openTabs   = writable([{ path: 'src/main.rs', content: '// NEXUS_READY\n', isDirty: false }]);
export const activeTab  = writable('src/main.rs');

// ── Neural Streaming ────────────────────────────────────────
export const isGhostTyping  = writable(false);
export const ghostBuffer    = writable('');

// Ghost-typing: array of { text, cls } line objects rendered safely
export const terminalLogs   = writable([]);   // { html: string (safe), raw: string }

// ── Patch Gate ──────────────────────────────────────────────
export const pendingPatch   = writable(null); // { path, content } | null

// ── Telemetry ───────────────────────────────────────────────
export const systemVitals   = writable({
    status:      'OFFLINE',
    cpu_usage:   0,
    ram_usage:   '0/0',
    resonance:   1.0,
    temperature: 0,
});

export const fileTreeTrigger = writable(0);

// ── WS connection singleton ─────────────────────────────────
export const wsReady = writable(false);
EOF

echo "  [OK] store.js"

# ─────────────────────────────────────────────────────────────
# 2. NexusWS.js — single shared WebSocket with reconnect
# ─────────────────────────────────────────────────────────────
cat > ui/src/lib/components/NexusWS.js << 'EOF'
/**
 * NexusWS.js — Singleton WebSocket hub with auto-reconnect.
 * All components import from here; only ONE socket is opened.
 */
import {
    terminalLogs, isGhostTyping, ghostBuffer,
    systemVitals, openTabs, fileTreeTrigger, wsReady, pendingPatch
} from '$lib/store';
import { get } from 'svelte/store';

let socket = null;
let reconnectTimer = null;
let messageHandlers = [];

/** Safely escape HTML — XSS fix for terminal output */
function esc(str) {
    return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

/** Convert ANSI escape codes → safe HTML spans */
function ansiToHtml(raw) {
    const safe = esc(raw);
    return safe
        .replace(/\x1b\[31m/g,           '<span class="ansi-red">')
        .replace(/\x1b\[32m/g,           '<span class="ansi-green">')
        .replace(/\x1b\[33m/g,           '<span class="ansi-yellow">')
        .replace(/\x1b\[36m/g,           '<span class="ansi-cyan">')
        .replace(/\x1b\[90m/g,           '<span class="ansi-gray">')
        .replace(/\x1b\[38;2;0;255;255m/g,'<span class="ansi-cyan-bold">')
        .replace(/\x1b\[0m/g,            '</span>');
}

export function pushTermLine(rawText, cssClass = '') {
    const html = ansiToHtml(rawText);
    terminalLogs.update(logs => {
        const next = [...logs, { html, cls: cssClass }];
        return next.length > 300 ? next.slice(next.length - 300) : next;
    });
}

export function sendDirective(text) {
    if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'directive', payload: text }));
    }
}

export function sendRaw(text) {
    if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(text + '\n');
    }
}

export function onMessage(fn) {
    messageHandlers.push(fn);
    return () => { messageHandlers = messageHandlers.filter(h => h !== fn); };
}

function handleMessage(event) {
    let data;
    try { data = JSON.parse(event.data); }
    catch { pushTermLine(event.data); return; }

    // Dispatch to all registered handlers
    messageHandlers.forEach(h => h(data));

    switch (data.type) {
        case 'telemetry':
            systemVitals.set(data.payload);
            break;

        case 'terminal':
            pushTermLine(data.payload);
            break;

        case 'ghost_start':
            isGhostTyping.set(true);
            ghostBuffer.set('');
            break;

        case 'ghost_token': {
            // Stream AI token into ghostBuffer + active editor
            const tok = data.payload;
            ghostBuffer.update(b => b + tok);
            openTabs.update(tabs => tabs.map(t =>
                t.path === get(activeTabStore) ? { ...t, content: t.content + tok, isDirty: true } : t
            ));
            break;
        }

        case 'ghost_end':
            isGhostTyping.set(false);
            fileTreeTrigger.update(n => n + 1);
            break;

        case 'patch_ready':
            pendingPatch.set(data.payload); // { path, content }
            break;

        case 'log':
            pushTermLine(`[${data.level ?? 'INFO'}] ${data.payload}`);
            break;
    }
}

// Need activeTab store ref — import carefully to avoid circular
import { activeTab as activeTabStore } from '$lib/store';

export function connectWS() {
    if (socket && socket.readyState <= WebSocket.OPEN) return;

    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    socket = new WebSocket(`${proto}//${window.location.host}/ws/nexus`);

    socket.onopen = () => {
        wsReady.set(true);
        pushTermLine('[NEXUS] WebSocket link established.', 'ansi-green');
        if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
    };

    socket.onmessage = handleMessage;

    socket.onclose = () => {
        wsReady.set(false);
        pushTermLine('[NEXUS] Link lost — reconnecting in 3s...', 'ansi-yellow');
        reconnectTimer = setTimeout(connectWS, 3000);
    };

    socket.onerror = () => {
        pushTermLine('[NEXUS] Socket error.', 'ansi-red');
    };
}
EOF

echo "  [OK] NexusWS.js"

# ─────────────────────────────────────────────────────────────
# 3. Terminal.svelte — XSS-safe, unified WS, reconnect aware
# ─────────────────────────────────────────────────────────────
cat > ui/src/lib/components/Terminal.svelte << 'EOF'
<script>
    import { onMount, tick } from 'svelte';
    import { terminalLogs, wsReady } from '$lib/store';
    import { sendRaw, pushTermLine } from '$lib/components/NexusWS.js';

    let termContainer;
    let inputCmd = '';

    $: if ($terminalLogs && termContainer) {
        tick().then(() => {
            termContainer.scrollTop = termContainer.scrollHeight;
        });
    }

    function handleKey(e) {
        if (e.key !== 'Enter') return;
        const cmd = inputCmd.trim();
        if (!cmd) return;
        // Echo locally — text is user-typed so safe to escape+show
        pushTermLine(`nexus_cli> ${cmd}`, 'ansi-gray');
        sendRaw(cmd);
        inputCmd = '';
    }
</script>

<div class="h-full w-full bg-[#050505] p-2 flex flex-col font-mono text-[10px] sm:text-xs">
    <!-- Connection badge -->
    <div class="flex justify-between items-center mb-1 text-[9px] border-b border-cyan-900/20 pb-1 shrink-0">
        <span class="text-zinc-600 tracking-widest uppercase">Terminal</span>
        <span class="flex items-center gap-1 {$wsReady ? 'text-emerald-500' : 'text-yellow-500 animate-pulse'}">
            <span class="w-1.5 h-1.5 rounded-full inline-block {$wsReady ? 'bg-emerald-500' : 'bg-yellow-500'}"></span>
            {$wsReady ? 'LINKED' : 'RECONNECTING'}
        </span>
    </div>

    <!-- Log lines — rendered via {@html} but ALL content is HTML-escaped first in NexusWS.js -->
    <div bind:this={termContainer}
         class="flex-1 overflow-y-auto scrollbar-hide break-all whitespace-pre-wrap leading-relaxed">
        {#each $terminalLogs as line}
            <!-- Safe: html field is always entity-escaped before ANSI span injection -->
            <div class="mb-0.5 {line.cls ?? ''}">{@html line.html}</div>
        {/each}
    </div>

    <!-- Input -->
    <div class="flex mt-1 border-t border-cyan-900/30 pt-1 shrink-0">
        <span class="mr-2 text-emerald-500">nexus_cli&gt;</span>
        <input
            class="flex-1 bg-transparent outline-none text-emerald-400 placeholder-zinc-700"
            bind:value={inputCmd}
            on:keydown={handleKey}
            placeholder="[SYSTEM OVERRIDE]..."
            autocomplete="off"
            spellcheck="false"
        />
    </div>
</div>

<style>
    /* ANSI color classes — used by ansiToHtml() in NexusWS.js */
    :global(.ansi-red)       { color: #f87171; }
    :global(.ansi-green)     { color: #34d399; }
    :global(.ansi-yellow)    { color: #fbbf24; }
    :global(.ansi-cyan)      { color: #22d3ee; }
    :global(.ansi-gray)      { color: #6b7280; }
    :global(.ansi-cyan-bold) { color: #67e8f9; font-weight: 700; }
</style>
EOF

echo "  [OK] Terminal.svelte"

# ─────────────────────────────────────────────────────────────
# 4. Editor.svelte — ghost typing, multi-lang, save shortcut
# ─────────────────────────────────────────────────────────────
cat > ui/src/lib/components/Editor.svelte << 'EOF'
<script>
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from 'codemirror';
    import { rust }       from '@codemirror/lang-rust';
    import { javascript } from '@codemirror/lang-javascript';
    import { python }     from '@codemirror/lang-python';
    import { markdown }   from '@codemirror/lang-markdown';
    import { oneDark }    from '@codemirror/theme-one-dark';
    import { openTabs, activeTab, isGhostTyping } from '$lib/store';
    import { pushTermLine } from '$lib/components/NexusWS.js';

    export let path = '';
    let editorElement;
    let view;
    let saveStatus = '';  // '', 'saving', 'saved', 'error'

    // Pick language extension by file extension
    function langExt(p) {
        if (p.endsWith('.rs'))              return rust();
        if (p.endsWith('.js') || p.endsWith('.ts') || p.endsWith('.svelte')) return javascript();
        if (p.endsWith('.py'))             return python();
        if (p.endsWith('.md') || p.endsWith('.toml')) return markdown();
        return [];
    }

    // Ghost-typing cursor decoration
    const ghostCursorTheme = EditorView.theme({
        '&.ghost-active .cm-cursor': {
            borderLeftColor: '#a855f7',
            borderLeftWidth: '2px',
            animation: 'ghost-blink 0.4s step-end infinite',
        }
    });

    function initEditor(content = '') {
        if (view) view.destroy();
        view = new EditorView({
            doc: content,
            extensions: [
                basicSetup,
                langExt(path),
                oneDark,
                ghostCursorTheme,
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        openTabs.update(tabs => tabs.map(t =>
                            t.path === path
                                ? { ...t, content: view.state.doc.toString(), isDirty: true }
                                : t
                        ));
                    }
                }),
                // Ctrl+S / Cmd+S save
                EditorView.domEventHandlers({
                    keydown(e) {
                        if ((e.ctrlKey || e.metaKey) && e.key === 's') {
                            e.preventDefault();
                            saveCurrentFile();
                        }
                    }
                }),
            ],
            parent: editorElement,
        });
    }

    // Sync store → editor when external code writes (AI ghost)
    $: if (path && view) {
        const newContent = $openTabs.find(t => t.path === path)?.content ?? '';
        if (view.state.doc.toString() !== newContent) {
            view.dispatch({
                changes: { from: 0, to: view.state.doc.length, insert: newContent },
            });
        }
        // Toggle ghost cursor class
        if ($isGhostTyping) {
            view.dom.classList.add('ghost-active');
        } else {
            view.dom.classList.remove('ghost-active');
        }
    }

    async function saveCurrentFile() {
        const tab = $openTabs.find(t => t.path === path);
        if (!tab) return;
        saveStatus = 'saving';
        try {
            const res = await fetch('/api/fs/write', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: tab.path, content: tab.content }),
            });
            if (!res.ok) throw new Error(await res.text());
            openTabs.update(tabs => tabs.map(t =>
                t.path === path ? { ...t, isDirty: false } : t
            ));
            saveStatus = 'saved';
            pushTermLine(`[FS] Saved → ${path}`, 'ansi-green');
        } catch (err) {
            saveStatus = 'error';
            pushTermLine(`[FS] Save failed: ${err.message}`, 'ansi-red');
        }
        setTimeout(() => saveStatus = '', 2000);
    }

    onMount(() => {
        const content = $openTabs.find(t => t.path === path)?.content ?? '';
        initEditor(content);
    });

    // Re-init editor when active path changes
    $: if (path && editorElement && view) {
        const content = $openTabs.find(t => t.path === path)?.content ?? '';
        const current = view.state.doc.toString();
        if (current !== content) initEditor(content);
    }

    onDestroy(() => view?.destroy());
</script>

<!-- Tab bar -->
<div class="flex items-center bg-[#0d1117] border-b border-cyan-900/20 text-[9px] font-mono overflow-x-auto scrollbar-hide shrink-0">
    {#each $openTabs as tab}
        <button
            on:click={() => activeTab.set(tab.path)}
            class="flex items-center gap-1.5 px-3 py-1.5 border-r border-cyan-900/10 whitespace-nowrap transition-colors
                   {tab.path === path ? 'bg-black text-cyan-400' : 'text-zinc-600 hover:text-zinc-300'}"
        >
            {#if tab.isDirty}
                <span class="w-1.5 h-1.5 rounded-full bg-yellow-500 shrink-0"></span>
            {/if}
            {tab.path.split('/').pop()}
        </button>
    {/each}

    <!-- Save status badge -->
    {#if saveStatus}
        <span class="ml-auto mr-2 px-2 py-0.5 rounded text-[8px] uppercase tracking-widest
            {saveStatus === 'saving' ? 'text-yellow-400' :
             saveStatus === 'saved'  ? 'text-emerald-400' : 'text-red-400'}">
            {saveStatus === 'saving' ? '● SAVING' : saveStatus === 'saved' ? '✓ SAVED' : '✕ ERROR'}
        </span>
    {/if}

    <!-- Manual save button -->
    <button
        on:click={saveCurrentFile}
        class="ml-auto mr-1 px-2 py-1 text-[8px] border border-cyan-900/40 text-cyan-700 hover:text-cyan-400 hover:border-cyan-500 transition-colors uppercase tracking-widest">
        Ctrl+S
    </button>
</div>

<div class="flex-1 overflow-hidden" bind:this={editorElement}></div>

<!-- Ghost typing indicator -->
{#if $isGhostTyping}
    <div class="flex items-center gap-2 bg-purple-950/30 border-t border-purple-800/40 px-3 py-1 text-[9px] text-purple-400 font-mono shrink-0">
        <span class="flex gap-0.5">
            <span class="w-1 h-1 rounded-full bg-purple-500 animate-bounce" style="animation-delay:0ms"></span>
            <span class="w-1 h-1 rounded-full bg-purple-500 animate-bounce" style="animation-delay:150ms"></span>
            <span class="w-1 h-1 rounded-full bg-purple-500 animate-bounce" style="animation-delay:300ms"></span>
        </span>
        AI WRITING CODE...
    </div>
{/if}

<style>
    :global(.cm-editor)  { height: 100%; font-size: 11px; }
    :global(.cm-scroller){ font-family: 'Fira Code', 'JetBrains Mono', monospace !important; }
    @keyframes ghost-blink {
        0%, 100% { opacity: 1; }
        50%       { opacity: 0; }
    }
</style>
EOF

echo "  [OK] Editor.svelte"

# ─────────────────────────────────────────────────────────────
# 5. Sidebar.svelte — file tree with create + delete
# ─────────────────────────────────────────────────────────────
cat > ui/src/lib/components/Sidebar.svelte << 'EOF'
<script>
    import { onMount } from 'svelte';
    import { activeTab, openTabs, fileTreeTrigger } from '$lib/store';
    import { pushTermLine } from '$lib/components/NexusWS.js';

    let files = [];
    let newFileName = '';
    let creating = false;
    let error = '';

    async function loadFiles() {
        try {
            const res = await fetch('/api/fs/list');
            if (res.ok) files = await res.json();
        } catch (e) {
            pushTermLine('[FS] Index scan failed.', 'ansi-red');
        }
    }

    async function selectFile(path) {
        if (path.endsWith('/')) return;
        // Already open?
        const existing = $openTabs.find(t => t.path === path);
        if (existing) { activeTab.set(path); return; }
        try {
            const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
            if (!res.ok) throw new Error('read failed');
            const data = await res.json();
            openTabs.update(tabs => [...tabs, { path, content: data.content, isDirty: false }]);
            activeTab.set(path);
        } catch {
            pushTermLine(`[FS] Could not open ${path}`, 'ansi-red');
        }
    }

    async function createFile() {
        const p = newFileName.trim();
        if (!p) return;
        error = '';
        try {
            const res = await fetch('/api/fs/write', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: p, content: '' }),
            });
            if (!res.ok) throw new Error(await res.text());
            newFileName = '';
            creating = false;
            await loadFiles();
            openTabs.update(tabs => [...tabs, { path: p, content: '', isDirty: false }]);
            activeTab.set(p);
            pushTermLine(`[FS] Created ${p}`, 'ansi-green');
        } catch (e) {
            error = e.message;
        }
    }

    async function deleteFile(path, e) {
        e.stopPropagation();
        if (!confirm(`Delete ${path}?`)) return;
        try {
            const res = await fetch(`/api/fs/delete?path=${encodeURIComponent(path)}`, { method: 'DELETE' });
            if (!res.ok) throw new Error(await res.text());
            openTabs.update(tabs => tabs.filter(t => t.path !== path));
            await loadFiles();
            pushTermLine(`[FS] Deleted ${path}`, 'ansi-yellow');
        } catch (e) {
            pushTermLine(`[FS] Delete failed: ${e.message}`, 'ansi-red');
        }
    }

    onMount(loadFiles);
    $: if ($fileTreeTrigger) loadFiles();
</script>

<div class="h-full bg-[#0d1117] flex flex-col font-mono text-[10px]">
    <!-- Header -->
    <div class="px-3 py-2 border-b border-cyan-900/30 flex justify-between items-center shrink-0">
        <span class="text-[9px] text-gray-500 tracking-widest uppercase">Sector_Index</span>
        <button
            on:click={() => creating = !creating}
            class="text-cyan-700 hover:text-cyan-400 text-base leading-none transition-colors"
            title="New file">+</button>
    </div>

    <!-- New file input -->
    {#if creating}
        <div class="px-2 py-2 border-b border-cyan-900/20 bg-black/40 shrink-0">
            <input
                bind:value={newFileName}
                placeholder="src/new_file.rs"
                class="w-full bg-transparent border border-cyan-800/50 text-cyan-300 px-2 py-1 text-[9px] outline-none focus:border-cyan-500"
                on:keydown={(e) => { if (e.key === 'Enter') createFile(); if (e.key === 'Escape') creating = false; }}
                autofocus
            />
            {#if error}
                <p class="text-red-500 text-[8px] mt-1">{error}</p>
            {/if}
        </div>
    {/if}

    <!-- File list -->
    <div class="flex-1 overflow-y-auto p-2 space-y-0.5 custom-scrollbar">
        {#each files as file}
            <button
                on:click={() => selectFile(file.path)}
                class="w-full text-left px-2 py-1.5 rounded transition-all flex items-center gap-2 group
                       {file.path === $activeTab
                           ? 'bg-cyan-900/30 text-cyan-400'
                           : 'text-gray-500 hover:bg-zinc-900 hover:text-gray-300'}"
            >
                <span class="shrink-0">{file.is_dir ? '📁' : '📄'}</span>
                <span class="truncate flex-1">{file.name}</span>
                {#if !file.is_dir}
                    <!-- dirty indicator from openTabs -->
                    {#if $openTabs.find(t => t.path === file.path)?.isDirty}
                        <span class="w-1.5 h-1.5 rounded-full bg-yellow-500 shrink-0"></span>
                    {/if}
                    <button
                        on:click={(e) => deleteFile(file.path, e)}
                        class="opacity-0 group-hover:opacity-100 text-red-800 hover:text-red-500 px-1 transition-opacity"
                        title="Delete">✕</button>
                {/if}
            </button>
        {/each}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar       { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e293b; }
</style>
EOF

echo "  [OK] Sidebar.svelte"

# ─────────────────────────────────────────────────────────────
# 6. MergeGate.svelte — wired to pendingPatch store
# ─────────────────────────────────────────────────────────────
cat > ui/src/lib/components/MergeGate.svelte << 'EOF'
<script>
    import { pendingPatch, openTabs, fileTreeTrigger } from '$lib/store';
    import { pushTermLine } from '$lib/components/NexusWS.js';

    async function applyPatch() {
        const patch = $pendingPatch;
        if (!patch) return;

        // Write into editor store
        openTabs.update(tabs => {
            const exists = tabs.find(t => t.path === patch.path);
            if (exists) {
                return tabs.map(t => t.path === patch.path
                    ? { ...t, content: patch.content, isDirty: true }
                    : t);
            }
            return [...tabs, { path: patch.path, content: patch.content, isDirty: true }];
        });

        // Persist to disk
        try {
            const res = await fetch('/api/fs/write', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: patch.path, content: patch.content }),
            });
            if (!res.ok) throw new Error(await res.text());
            openTabs.update(tabs => tabs.map(t =>
                t.path === patch.path ? { ...t, isDirty: false } : t
            ));
            fileTreeTrigger.update(n => n + 1);
            pushTermLine(`[PATCH] Applied → ${patch.path}`, 'ansi-green');
        } catch (e) {
            pushTermLine(`[PATCH] Write failed: ${e.message}`, 'ansi-red');
        }

        pendingPatch.set(null);
    }

    function rejectPatch() {
        pushTermLine(`[PATCH] Rejected by operator.`, 'ansi-yellow');
        pendingPatch.set(null);
    }
</script>

{#if $pendingPatch}
<div class="fixed bottom-24 right-8 w-80 bg-[#050505] border border-yellow-500/50
            shadow-[0_0_20px_rgba(234,179,8,0.2)] z-[100] p-4 font-mono">
    <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
            <div class="w-2 h-2 bg-yellow-500 animate-pulse rounded-full"></div>
            <span class="text-[10px] text-yellow-500 font-bold uppercase tracking-widest">Patch Detected</span>
        </div>
        <span class="text-[8px] text-gray-500 truncate max-w-[120px]">{$pendingPatch.path}</span>
    </div>

    <!-- Diff preview — first 8 lines -->
    <pre class="text-[8px] text-gray-400 bg-black/60 border border-gray-800 p-2 mb-4 max-h-20 overflow-hidden leading-tight">{$pendingPatch.content.split('\n').slice(0,8).join('\n')}{$pendingPatch.content.split('\n').length > 8 ? '\n...' : ''}</pre>

    <div class="flex gap-2">
        <button
            on:click={applyPatch}
            class="flex-1 bg-yellow-500/10 border border-yellow-500 text-yellow-500 text-[10px] py-1.5
                   font-bold hover:bg-yellow-500 hover:text-black transition-all">
            COMMIT CHANGE
        </button>
        <button
            on:click={rejectPatch}
            class="px-3 bg-red-950/20 border border-red-900 text-red-700 text-[10px]
                   hover:bg-red-900 hover:text-white transition-colors">
            ✕
        </button>
    </div>
</div>
{/if}
EOF

echo "  [OK] MergeGate.svelte"

# ─────────────────────────────────────────────────────────────
# 7. VitalsModule.svelte — safe, no change from original but clean
# ─────────────────────────────────────────────────────────────
cat > ui/src/lib/components/VitalsModule.svelte << 'EOF'
<script>
    export let vitals = { status: 'IDLE', cpu_usage: 0, ram_usage: '0/0', temperature: 0, resonance: 1.0 };
</script>

<div class="flex flex-col gap-1 font-mono text-[9px]">
    <div class="flex justify-between border-b border-cyan-900/20 pb-1 mb-1">
        <span class="text-zinc-600 uppercase tracking-widest">Telemetry</span>
        <span class="{vitals.status === 'CRITICAL' ? 'text-pink-600 animate-pulse' :
                      vitals.status === 'ONLINE'   ? 'text-emerald-500' : 'text-zinc-500'}">
            {vitals.status}
        </span>
    </div>
    <div class="flex justify-between">
        <span class="text-zinc-500">CPU</span>
        <span class="text-cyan-400">{vitals.cpu_usage?.toFixed(1) ?? '0.0'}%</span>
    </div>
    <div class="flex justify-between">
        <span class="text-zinc-500">RAM</span>
        <span class="text-cyan-400">{vitals.ram_usage}</span>
    </div>
    <div class="flex justify-between">
        <span class="text-zinc-500">HEAT</span>
        <span class="{vitals.temperature > 60 ? 'text-pink-500' : 'text-cyan-400'}">
            {vitals.temperature?.toFixed(1) ?? '0.0'}°C
        </span>
    </div>
    <div class="flex justify-between mt-1">
        <span class="text-zinc-500">RESONANCE</span>
        <span class="text-purple-400">{vitals.resonance?.toFixed(3) ?? '1.000'}</span>
    </div>
    <!-- Mini bars -->
    <div class="mt-1 space-y-1">
        <div class="h-0.5 bg-zinc-900 rounded overflow-hidden">
            <div class="h-full bg-cyan-500 transition-all duration-500" style="width:{vitals.cpu_usage ?? 0}%"></div>
        </div>
        <div class="h-0.5 bg-zinc-900 rounded overflow-hidden">
            <div class="h-full bg-purple-500 transition-all duration-500"
                 style="width:{(vitals.resonance ?? 1)*100}%"></div>
        </div>
    </div>
</div>
EOF

echo "  [OK] VitalsModule.svelte"

# ─────────────────────────────────────────────────────────────
# 8. +page.svelte — full DevOps IDE layout, wired together
# ─────────────────────────────────────────────────────────────
cat > ui/src/routes/+page.svelte << 'EOF'
<script>
    import { onMount } from 'svelte';
    import { activeTab, isGhostTyping, ghostBuffer, systemVitals, pendingPatch } from '$lib/store';
    import { connectWS, sendDirective, pushTermLine } from '$lib/components/NexusWS.js';

    import VitalsModule  from '$lib/components/VitalsModule.svelte';
    import Sidebar       from '$lib/components/Sidebar.svelte';
    import Editor        from '$lib/components/Editor.svelte';
    import Terminal      from '$lib/components/Terminal.svelte';
    import MergeGate     from '$lib/components/MergeGate.svelte';

    let directive = '';
    let sending   = false;
    let aiPanel   = true; // toggle AI response panel

    async function submitDirective() {
        const text = directive.trim().replace(/</g, '&lt;').replace(/>/g, '&gt;');
        if (!text || sending) return;
        sending   = true;
        directive = '';
        pushTermLine(`[DIRECTIVE] ${text}`, 'ansi-cyan');
        sendDirective(text);
        // Also POST for REST-style backends
        try {
            await fetch('/api/prompt', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ prompt: text }),
            });
        } catch {}
        sending = false;
    }

    function handleKey(e) {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            submitDirective();
        }
    }

    onMount(() => {
        connectWS();
    });
</script>

<!-- ── ROOT GRID ────────────────────────────────────────────── -->
<div class="h-screen w-screen bg-[#060608] text-cyan-400 font-mono
            grid grid-cols-[220px_1fr_320px] grid-rows-[40px_1fr_180px_36px]
            gap-px overflow-hidden">

    <!-- ── TOP BAR ─────────────────────────────────────────── -->
    <header class="col-span-3 row-span-1 bg-[#0a0b0e] border-b border-cyan-900/30
                   flex items-center justify-between px-4 gap-4">
        <div class="flex items-center gap-3">
            <div class="w-2 h-2 rounded-full {$systemVitals.status === 'ONLINE' ? 'bg-emerald-500 shadow-[0_0_6px_#22c55e]' : 'bg-zinc-700'}"></div>
            <span class="text-[11px] font-bold tracking-[3px] text-cyan-300 uppercase">
                NEXUS SINGULARITY
            </span>
            <span class="text-[9px] text-zinc-600">v1.3.0 // ARM64</span>
        </div>

        <!-- Vitals strip in header -->
        <div class="flex items-center gap-6 text-[9px] text-zinc-500 font-mono">
            <span>CPU <span class="text-cyan-400">{$systemVitals.cpu_usage?.toFixed(1) ?? '0.0'}%</span></span>
            <span>RAM <span class="text-cyan-400">{$systemVitals.ram_usage}</span></span>
            <span>HEAT <span class="{$systemVitals.temperature > 60 ? 'text-pink-400' : 'text-cyan-400'}">{$systemVitals.temperature?.toFixed(0) ?? '0'}°C</span></span>
            <span>RES <span class="text-purple-400">{$systemVitals.resonance?.toFixed(3) ?? '1.000'}</span></span>
        </div>

        <div class="flex items-center gap-2">
            <!-- AI panel toggle -->
            <button
                on:click={() => aiPanel = !aiPanel}
                class="text-[9px] px-2 py-1 border transition-colors
                       {aiPanel ? 'border-purple-700 text-purple-400' : 'border-zinc-800 text-zinc-600'}">
                AI PANEL
            </button>
            <!-- Ghost status -->
            {#if $isGhostTyping}
                <span class="text-[9px] text-purple-400 animate-pulse tracking-widest">◈ COGNITION ACTIVE</span>
            {:else}
                <span class="text-[9px] text-zinc-700 tracking-widest">◇ HIVE IDLE</span>
            {/if}
        </div>
    </header>

    <!-- ── SIDEBAR ─────────────────────────────────────────── -->
    <aside class="row-start-2 row-end-4 col-start-1 bg-[#0d1117] border-r border-cyan-900/20 overflow-hidden">
        <Sidebar />
    </aside>

    <!-- ── EDITOR ─────────────────────────────────────────── -->
    <main class="row-start-2 col-start-2
                 {aiPanel ? '' : 'col-end-4'}
                 bg-black border-r border-cyan-900/10 flex flex-col overflow-hidden">
        {#if $activeTab}
            <Editor path={$activeTab} />
        {:else}
            <div class="flex-1 flex items-center justify-center text-zinc-700 text-sm">
                SELECT A FILE TO EDIT
            </div>
        {/if}
    </main>

    <!-- ── AI RESPONSE / GHOST BUFFER ─────────────────────── -->
    {#if aiPanel}
    <div class="row-start-2 col-start-3 bg-[#050508] border-l border-cyan-900/20
                flex flex-col overflow-hidden font-mono text-[10px]">
        <div class="flex items-center justify-between px-3 py-2 border-b border-cyan-900/20 shrink-0">
            <span class="text-[9px] text-purple-400 tracking-widest uppercase">Neural Link</span>
            {#if $isGhostTyping}
                <div class="flex items-center gap-1 text-purple-500 text-[8px]">
                    <span class="animate-ping w-1.5 h-1.5 rounded-full bg-purple-500 inline-block"></span>
                    STREAMING
                </div>
            {/if}
        </div>
        <div class="flex-1 overflow-y-auto p-3 whitespace-pre-wrap text-emerald-400/90
                    leading-relaxed custom-scroll text-[10px]">
            {$ghostBuffer || '>> UPLINK STABLE. AWAITING DIRECTIVE...'}
        </div>
    </div>
    {/if}

    <!-- ── TERMINAL ────────────────────────────────────────── -->
    <div class="row-start-3 col-start-2 {aiPanel ? '' : 'col-end-4'}
                border-t border-cyan-900/30 overflow-hidden">
        <Terminal />
    </div>

    <!-- ── TERMINAL RIGHT (vitals detail) ─────────────────── -->
    {#if aiPanel}
    <div class="row-start-3 col-start-3 bg-[#050508] border-t border-l border-cyan-900/20
                p-3 overflow-hidden">
        <VitalsModule vitals={$systemVitals} />
    </div>
    {/if}

    <!-- ── DIRECTIVE INPUT ────────────────────────────────── -->
    <footer class="col-span-3 row-start-4 bg-[#0a0b0e] border-t border-cyan-900/40
                   flex items-center gap-3 px-4">
        <span class="text-[9px] text-cyan-800 uppercase tracking-widest shrink-0">Directive ▶</span>
        <textarea
            bind:value={directive}
            on:keydown={handleKey}
            rows="1"
            class="flex-1 bg-transparent outline-none text-cyan-200 text-xs resize-none
                   placeholder-zinc-700 leading-tight py-1"
            placeholder="ENTER DIRECTIVE FOR CORTEX LOOP... (Enter to send, Shift+Enter for newline)"
        ></textarea>
        <button
            on:click={submitDirective}
            disabled={sending}
            class="shrink-0 px-3 py-1 text-[9px] border transition-all uppercase tracking-widest
                   {sending
                       ? 'border-zinc-800 text-zinc-700 cursor-wait'
                       : 'border-cyan-700 text-cyan-500 hover:bg-cyan-900/30 hover:border-cyan-400'}">
            {sending ? '...' : 'SEND'}
        </button>
    </footer>
</div>

<!-- ── PATCH GATE overlay ─────────────────────────────────── -->
<MergeGate />

<style>
    :global(body)  { margin: 0; background: #060608; overflow: hidden; }
    .custom-scroll::-webkit-scrollbar       { width: 3px; }
    .custom-scroll::-webkit-scrollbar-thumb { background: #1e1b4b; }
</style>
EOF

echo "  [OK] +page.svelte"

# ─────────────────────────────────────────────────────────────
# 9. Install missing CodeMirror language packages
# ─────────────────────────────────────────────────────────────
echo ""
echo ">> [NPM] Installing missing language extensions..."
cd ui && npm install --save \
    @codemirror/lang-javascript \
    @codemirror/lang-python \
    @codemirror/lang-markdown 2>/dev/null && cd ..

echo ""
echo "════════════════════════════════════════════════════════"
echo "  NEXUS UPGRADE COMPLETE"
echo ""
echo "  Files written:"
echo "    ui/src/lib/store.js"
echo "    ui/src/lib/components/NexusWS.js"
echo "    ui/src/lib/components/Terminal.svelte    ← XSS fixed"
echo "    ui/src/lib/components/Editor.svelte      ← ghost type + save"
echo "    ui/src/lib/components/Sidebar.svelte     ← create/delete"
echo "    ui/src/lib/components/MergeGate.svelte   ← wired to store"
echo "    ui/src/lib/components/VitalsModule.svelte"
echo "    ui/src/routes/+page.svelte               ← full IDE layout"
echo ""
echo "  Backend endpoints expected:"
echo "    WS  /ws/nexus                 (was /ws/ghost)"
echo "    GET  /api/fs/list"
echo "    GET  /api/fs/read?path="
echo "    POST /api/fs/write            {path, content}"
echo "    DEL  /api/fs/delete?path="
echo "    POST /api/prompt              {prompt}"
echo "    GET  /api/vitals"
echo ""
echo "  WS message protocol (server → client JSON):"
echo "    { type: 'telemetry',   payload: vitalsObj }"
echo "    { type: 'terminal',    payload: 'string'  }"
echo "    { type: 'ghost_start'                     }"
echo "    { type: 'ghost_token', payload: 'string'  }"
echo "    { type: 'ghost_end'                       }"
echo "    { type: 'patch_ready', payload: { path, content } }"
echo "    { type: 'log', level: 'INFO|ERROR', payload: 'string' }"
echo ""
echo "  To build:"
echo "    cd ui && npm run build"
echo "    cd .. && cargo run --release"
echo "════════════════════════════════════════════════════════"
EOF
