<script>
    // ════════════════════════════════════════════════════════════
    //  Sidebar — File tree + open/create controls
    // ════════════════════════════════════════════════════════════
    import { onMount } from 'svelte';
    import { fileTreeTrigger } from '$lib/core/store.svelte.js';

    let { onFileSelect = null, onCreateFile = null } = $props();

    let files       = $state([]);
    let scanning    = $state(false);
    let currentPath = $state('.');
    let breadcrumb  = $state(['.']);
    let newFileName = $state('');
    let creating    = $state(false);
    let createErr   = $state('');

    const FILE_ICONS = {
        rs:'🦀', js:'🟨', ts:'🔷', svelte:'🔶', toml:'⚙',
        md:'📝', json:'📋', zig:'⚡', css:'🎨', html:'🌐',
        sh:'💻', py:'🐍', lock:'🔒', env:'🔑',
    };

    function fileIcon(name) {
        const ext = name.split('.').at(-1) ?? '';
        return FILE_ICONS[ext] ?? '📄';
    }

    async function scan(path = currentPath) {
        scanning = true;
        createErr = '';
        try {
            const r = await fetch(`/api/fs/list?path=${encodeURIComponent(path)}`);
            if (r.ok) { files = await r.json(); currentPath = path; }
        } catch(e) { console.error('SIDEBAR_FAULT', e); }
        finally { scanning = false; }
    }

    function enterDir(path) {
        breadcrumb = [...breadcrumb, path];
        scan(path);
    }

    function goUp() {
        if (breadcrumb.length <= 1) return;
        const b = [...breadcrumb];
        b.pop();
        breadcrumb = b;
        scan(b.at(-1));
    }

    async function handleCreate(e) {
        e.preventDefault();
        const name = newFileName.trim();
        if (!name || creating) return;
        const fullPath = currentPath === '.' ? name : `${currentPath}/${name}`;
        creating = true;
        createErr = '';
        try {
            if (onCreateFile) {
                await onCreateFile(fullPath);
            } else {
                await fetch('/api/fs/write', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ path: fullPath, content: '' })
                });
            }
            newFileName = '';
            await scan();
        } catch(e) { createErr = e.message; }
        finally { creating = false; }
    }

    $effect(() => { if (fileTreeTrigger.value) scan(); });
    onMount(() => scan('.'));
</script>

<div class="sidebar-root">
    <!-- Breadcrumb path -->
    <div class="path-bar">
        {#if breadcrumb.length > 1}
            <button class="up-btn" onclick={goUp} title="Go up">‹ UP</button>
        {/if}
        <span class="path-label">
            {currentPath === '.' ? '~/' : '.../' + currentPath.split('/').at(-1) + '/'}
        </span>
        {#if scanning}
            <span class="spin">⟳</span>
        {/if}
    </div>

    <!-- Create new file -->
    <div class="create-zone">
        <div class="zone-label">NEW FILE</div>
        <form onsubmit={handleCreate} class="create-row">
            <input
                bind:value={newFileName}
                placeholder="e.g. module.rs"
                class="create-inp"
                disabled={creating}
                autocomplete="off"
                spellcheck="false"
            />
            <button type="submit" class="create-btn" disabled={creating} title="Create">
                {creating ? '…' : '+'}
            </button>
        </form>
        {#if createErr}
            <div class="create-err">{createErr}</div>
        {/if}
    </div>

    <!-- File tree -->
    <div class="tree-label">DNA_STRUCTURE</div>
    <div class="tree-body">
        {#if files.length === 0 && !scanning}
            <div class="tree-empty">EMPTY DIRECTORY</div>
        {/if}
        <!-- Dirs first -->
        {#each files.filter(f => f.is_dir) as d}
            <button class="tree-item dir-item" onclick={() => enterDir(d.path)} title={d.path}>
                <span class="ti-icon">📁</span>
                <span class="ti-name">{d.name}/</span>
                <span class="ti-arrow">›</span>
            </button>
        {/each}
        <!-- Then files -->
        {#each files.filter(f => !f.is_dir) as f}
            <button
                class="tree-item file-item"
                onclick={() => onFileSelect && onFileSelect(f.path)}
                title={f.path}
            >
                <span class="ti-icon">{fileIcon(f.name)}</span>
                <span class="ti-name">{f.name}</span>
            </button>
        {/each}
    </div>

    <button class="refresh-bar" onclick={() => scan()} title="Refresh file tree">
        ⟳ REFRESH DNA
    </button>
</div>

<style>
    .sidebar-root {
        display: flex; flex-direction: column;
        height: 100%; background: #020208; overflow: hidden;
    }

    /* Path bar */
    .path-bar {
        flex: none; display: flex; align-items: center; gap: 6px;
        padding: 5px 8px; border-bottom: 1px solid rgba(0,255,255,.05);
        font-family: 'JetBrains Mono', monospace; font-size: 9px;
        background: #000;
    }
    .up-btn {
        background: rgba(0,255,255,.08); border: 1px solid rgba(0,255,255,.15);
        color: #22d3ee; font-size: 8px; padding: 2px 7px; cursor: pointer;
        font-family: 'JetBrains Mono', monospace; transition: background .12s;
    }
    .up-btn:hover { background: rgba(0,255,255,.18); }
    .path-label { color: rgba(0,255,255,.4); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .spin { color: #34d399; animation: spin 1s linear infinite; display: inline-block; }
    @keyframes spin { to { transform: rotate(360deg); } }

    /* Create zone */
    .create-zone { flex: none; padding: 7px 8px; border-bottom: 1px solid rgba(0,255,255,.05); }
    .zone-label { font-size: 7px; font-weight: 900; letter-spacing: .14em; color: rgba(0,255,255,.25); margin-bottom: 5px; }
    .create-row { display: flex; gap: 3px; }
    .create-inp {
        flex: 1; background: #000; border: 1px solid rgba(0,255,255,.12);
        color: #22d3ee; font-size: 10px; padding: 4px 7px; outline: none;
        font-family: 'JetBrains Mono', monospace; transition: border-color .15s;
    }
    .create-inp:focus { border-color: rgba(0,255,255,.4); }
    .create-inp::placeholder { color: rgba(0,255,255,.2); }
    .create-btn {
        width: 28px; background: rgba(34,211,238,.1); border: 1px solid rgba(0,255,255,.2);
        color: #22d3ee; font-size: 16px; cursor: pointer; font-weight: 700;
        transition: background .12s; display: flex; align-items: center; justify-content: center;
    }
    .create-btn:hover:not(:disabled) { background: rgba(34,211,238,.25); }
    .create-btn:disabled { opacity: .4; cursor: not-allowed; }
    .create-err { font-size: 8px; color: #f87171; margin-top: 3px; }

    /* Tree */
    .tree-label {
        flex: none; padding: 5px 8px 4px;
        font-size: 7px; font-weight: 900; letter-spacing: .14em;
        color: rgba(0,255,255,.2); border-bottom: 1px solid rgba(0,255,255,.04);
    }
    .tree-body { flex: 1; overflow-y: auto; }
    .tree-body::-webkit-scrollbar { width: 2px; }
    .tree-body::-webkit-scrollbar-thumb { background: #1e3a4a; }
    .tree-empty { padding: 16px 10px; font-size: 9px; color: #374151; font-style: italic; font-family:'JetBrains Mono',monospace; }

    .tree-item {
        display: flex; align-items: center; gap: 7px;
        width: 100%; text-align: left; padding: 4px 10px;
        border: none; background: transparent;
        font-size: 10px; font-family: 'JetBrains Mono', monospace;
        cursor: pointer; transition: all .08s;
        border-left: 2px solid transparent;
    }
    .dir-item { color: rgba(167,139,250,.5); font-weight: 700; }
    .dir-item:hover { background: rgba(167,139,250,.07); color: #a78bfa; border-left-color: #a78bfa; }
    .file-item { color: rgba(0,255,255,.4); }
    .file-item:hover { background: rgba(0,255,255,.06); color: #22d3ee; border-left-color: #22d3ee; }
    .ti-icon { flex: none; font-size: 11px; }
    .ti-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .ti-arrow { flex: none; color: rgba(255,255,255,.15); font-size: 11px; }

    .refresh-bar {
        flex: none; width: 100%; padding: 5px;
        background: transparent; border-top: 1px solid rgba(0,255,255,.04);
        color: rgba(0,255,255,.18); font-size: 8px; font-weight: 900;
        letter-spacing: .12em; cursor: pointer; font-family:'JetBrains Mono',monospace;
        transition: all .12s;
    }
    .refresh-bar:hover { color: rgba(0,255,255,.5); background: rgba(0,255,255,.02); }
</style>
