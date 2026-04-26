<script>
    // HistoryPanel.svelte — v1.1 [CORRECT_ENDPOINT]
    // FIX: Was fetching /api/memories and filtering by keyword — fragile.
    //      Now fetches /api/lazarus/snapshots directly for reliable data.
    // FIX: restoreSnap uses uplink for consistent auth header.
    import { uplink } from '$lib/services/uplink.js';
    import { onMount } from 'svelte';

    let snapshots = $state([]);
    let loading   = $state(true);
    let restoring = $state(null);
    let snapMsg   = $state('');

    async function loadSnaps() {
        loading = true;
        snapMsg = 'Scanning vault...';
        try {
            const r = await uplink.fetch('/api/lazarus/snapshots');
            if (r.ok) {
                const all = await r.json();
                // Newest first, max 50
                snapshots = [...all].reverse().slice(0, 50);
                snapMsg   = snapshots.length === 0 ? 'No snapshots in vault.' : '';
            } else {
                snapMsg = `HTTP ${r.status} — vault unavailable`;
                snapshots = [];
            }
        } catch (e) {
            snapshots = [];
            snapMsg   = 'Vault scan failed.';
        } finally {
            loading = false;
            setTimeout(() => snapMsg = '', 3000);
        }
    }

    async function restoreSnap(hash) {
        if (!hash) return;
        restoring = hash;
        snapMsg   = `Initiating Temporal Restore: ${hash.slice(0, 8)}…`;
        try {
            await uplink.post('/api/task', {
                directive: `restore snapshot ${hash}`,
            });
            snapMsg = `Restore queued for ${hash.slice(0, 8)} — check terminal.`;
        } catch (e) {
            snapMsg = `!! [ERROR] ${e.message}`;
        } finally {
            restoring = null;
            setTimeout(() => snapMsg = '', 5000);
        }
    }

    onMount(loadSnaps);
</script>

<div class="panel-scroll history-panel">
    <div class="hist-hdr">
        <div class="hist-title">LAZARUS_VAULT</div>
        <button onclick={loadSnaps} class="refresh-btn" title="Refresh snapshots">⟳</button>
    </div>

    {#if snapMsg}
        <div class="snap-msg-box">{snapMsg}</div>
    {/if}

    {#if loading}
        <div class="snap-loading">SCANNING VAULT…</div>
    {:else}
        <div class="snap-list">
            {#each snapshots as snap (snap.id ?? snap.hash)}
                <div class="snap-row">
                    <div class="snap-info">
                        <div class="snap-path" title={snap.path}>{snap.path ?? 'unknown'}</div>
                        <div class="snap-meta">
                            <span class="snap-hash">{(snap.hash ?? snap.id ?? '').slice(0, 8)}</span>
                            <span class="snap-tag">{snap.directive ?? ''}</span>
                        </div>
                        <div class="snap-ts">{snap.timestamp ?? ''}</div>
                    </div>
                    <button
                        class="snap-restore"
                        disabled={restoring === (snap.id ?? snap.hash)}
                        onclick={() => restoreSnap(snap.hash ?? snap.id ?? '')}
                    >
                        {restoring === (snap.id ?? snap.hash) ? '…' : 'RESTORE'}
                    </button>
                </div>
            {:else}
                <div class="snap-empty">NO SNAPSHOTS — run a directive to create one</div>
            {/each}
        </div>
    {/if}
</div>

<style>
    .panel-scroll   { flex:1; overflow-y:auto; padding:0; height:100%; display:flex; flex-direction:column; }
    .hist-hdr       { display:flex; align-items:center; justify-content:space-between; padding:14px 16px; border-bottom:1px solid rgba(167,139,250,.1); flex:none; }
    .hist-title     { font-size:11px; font-weight:900; letter-spacing:.15em; color:#a78bfa; }
    .refresh-btn    { background:none; border:none; color:rgba(167,139,250,.5); font-size:16px; cursor:pointer; transition:color .12s; }
    .refresh-btn:hover { color:#a78bfa; }
    .snap-msg-box   { padding:8px 16px; font-size:9px; color:#a78bfa; flex:none; }
    .snap-loading   { padding:32px; text-align:center; font-size:9px; color:rgba(167,139,250,.3); letter-spacing:.1em; animation:pulse 1.5s infinite; }
    @keyframes pulse { 50%{opacity:.3;} }
    .snap-list      { flex:1; overflow-y:auto; padding:8px; display:flex; flex-direction:column; gap:4px; }
    .snap-row       { display:flex; align-items:center; gap:10px; padding:8px 10px; border:1px solid rgba(167,139,250,.1); background:rgba(167,139,250,.02); transition:border-color .12s; }
    .snap-row:hover { border-color:rgba(167,139,250,.25); }
    .snap-info      { flex:1; min-width:0; }
    .snap-path      { font-size:10px; color:rgba(255,255,255,.6); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; margin-bottom:2px; }
    .snap-meta      { display:flex; gap:8px; align-items:center; }
    .snap-hash      { font-size:8px; color:#a78bfa; font-family:'JetBrains Mono',monospace; }
    .snap-tag       { font-size:8px; color:rgba(255,255,255,.25); }
    .snap-ts        { font-size:7px; color:rgba(167,139,250,.3); margin-top:2px; }
    .snap-restore   { flex:none; padding:4px 10px; border:1px solid rgba(167,139,250,.3); background:transparent; color:#a78bfa; font-size:8px; font-weight:900; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .12s; }
    .snap-restore:hover:not(:disabled) { background:rgba(167,139,250,.12); }
    .snap-restore:disabled { opacity:.4; cursor:not-allowed; }
    .snap-empty     { padding:32px; text-align:center; font-size:9px; color:rgba(167,139,250,.2); }
</style>
