<script>
    import { onMount } from 'svelte';

    let snapshots = $state([]);
    let loading   = $state(true);
    let restoring = $state(null);
    let snapMsg   = $state('');

    async function loadSnaps() {
        loading = true;
        snapMsg = 'Scanning vault...';
        try {
            const r = await fetch('/api/memories');
            if (r.ok) {
                const all = await r.json();
                snapshots = all
                    .filter(m => m.directive?.toLowerCase().includes('snapshot') || m.origin?.toLowerCase().includes('lazarus'))
                    .slice(0, 20)
                    .reverse();
                snapMsg = snapshots.length === 0 ? 'No snapshots found in memory.' : '';
            }
        } catch {
            snapshots = [];
            snapMsg = 'Vault scan failed.';
        } finally {
            loading = false;
            setTimeout(() => snapMsg = '', 3000);
        }
    }

    async function restoreSnap(hash) {
        restoring = hash;
        snapMsg = `Initiating Temporal Restore: ${hash.slice(0,8)}...`;
        try {
            await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive: `restore snapshot ${hash}` })
            });
        } catch(e) {
            snapMsg = `!! [ERROR] ${e.message}`;
        } finally {
            restoring = null;
            setTimeout(() => snapMsg = '', 4000);
        }
    }

    onMount(loadSnaps);
</script>

<div class="panel-scroll history-panel">
    <div class="hist-hdr">
        <div class="hist-title">LAZARUS_VAULT</div>
        <button onclick={loadSnaps} class="refresh-btn">⟳</button>
    </div>
    {#if snapMsg}<div class="snap-msg-box">{snapMsg}</div>{/if}
    <div class="snap-list">
        {#each snapshots as snap}
            <div class="snap-row">
                <div class="snap-info">
                    <div class="snap-path">{snap.directive ?? snap.path ?? 'unknown'}</div>
                    <div class="snap-ts">{snap.timestamp ?? ''}</div>
                </div>
                <button class="snap-restore" disabled={restoring === snap.id} onclick={() => restoreSnap(snap.id?.toString() ?? '')}>RESTORE</button>
            </div>
        {:else}
            {#if !loading}
                <div class="snap-empty">NO SNAPSHOTS — run a directive to create one</div>
            {/if}
        {/each}
    </div>
</div>

<style>
    .panel-scroll { flex:1; overflow-y:auto; padding:18px; height:100%; display:flex; flex-direction:column;}
    .hist-hdr { display:flex; align-items:center; justify-content:space-between; padding:14px 16px; border-bottom:1px solid rgba(167,139,250,.1); }
    .hist-title { font-size:11px; font-weight:900; letter-spacing:.15em; color:#a78bfa; }
    .refresh-btn { background:none; border:none; color:rgba(167,139,250,.5); font-size:16px; cursor:pointer; }
    .snap-msg-box { padding:8px 16px; font-size:9px; color:#a78bfa; }
    .snap-list { flex:1; overflow-y:auto; padding:8px; display:flex; flex-direction:column; gap:4px; }
    .snap-row { display:flex; align-items:center; gap:10px; padding:8px 10px; border:1px solid rgba(167,139,250,.1); background:rgba(167,139,250,.02); }
    .snap-info { flex:1; min-width:0; }
    .snap-path { font-size:10px; color:rgba(255,255,255,.5); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
    .snap-ts { font-size:8px; color:rgba(167,139,250,.4); }
    .snap-restore { flex:none; padding:4px 10px; border:1px solid rgba(167,139,250,.3); background:transparent; color:#a78bfa; font-size:8px; font-weight:900; cursor:pointer; font-family:'JetBrains Mono',monospace; }
    .snap-empty { padding:32px; text-align:center; font-size:9px; color:rgba(167,139,250,.2); }
</style>
