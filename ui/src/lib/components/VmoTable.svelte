<script>
    // VmoTable.svelte — v1.2
    // IMPROVE: Pauses polling via visibilitychange event listener so the
    //          interval doesn't fire at all when the tab is hidden —
    //          not just skip the fetch. Saves ~1 wakeup/2s on ARM64.
    import { onMount, onDestroy } from 'svelte';
    import { fly } from 'svelte/transition';
    import { vmoTelemetry } from '$lib/core/store.svelte.js';

    let vmos        = $state([]);
    let loading     = $state(true);
    let errorMsg    = $state('');
    let pollInterval;

    async function fetchVmos() {
        if (document.visibilityState === 'hidden') return;
        try {
            const r = await fetch('/api/vmos');
            if (r.ok) {
                const data = await r.json();
                vmos     = data.vmos ?? [];
                errorMsg = '';
            } else {
                errorMsg = `HTTP ${r.status} — VMO data unavailable`;
            }
        } catch (e) {
            errorMsg = 'Uplink to Kernel VMO Table severed.';
        } finally {
            loading = false;
        }
    }

    function onVisChange() {
        if (document.visibilityState === 'visible') fetchVmos();
    }

    onMount(() => {
        fetchVmos();
        pollInterval = setInterval(fetchVmos, 2000);
        document.addEventListener('visibilitychange', onVisChange);
    });

    onDestroy(() => {
        clearInterval(pollInterval);
        document.removeEventListener('visibilitychange', onVisChange);
    });
</script>

<div class="vmo-container" in:fly={{ y:10, duration:200 }}>
    <div class="header">
        <span class="title">◈ VIRTUAL MEMORY OBJECTS (VMO)</span>
        <span class="active-count">ACTIVE: <span class="highlight">{vmoTelemetry.active_vmos}</span></span>
    </div>

    {#if loading && vmos.length === 0}
        <div class="status-msg" style="color:#22d3ee">SCANNING KERNEL MEMORY MAP...</div>
    {:else if errorMsg}
        <div class="status-msg" style="color:#f87171">⚠ {errorMsg}</div>
    {:else if vmos.length === 0}
        <div class="status-msg" style="color:#64748b">NO ACTIVE VMO ALLOCATIONS DETECTED.</div>
    {:else}
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th>HANDLE</th><th>SIZE (B)</th><th>COMMITTED</th>
                        <th>RAM USED</th><th>COW</th><th>PARENT</th>
                    </tr>
                </thead>
                <tbody>
                    {#each vmos as vmo}
                        <tr class:is-cow={vmo.is_cow}>
                            <td class="handle">{vmo.handle}</td>
                            <td class="numeric">{(vmo.size_bytes||0).toLocaleString()}</td>
                            <td class="numeric">{vmo.committed_pages}</td>
                            <td class="ram-highlight">{(vmo.memory_used_kb||0).toLocaleString()} KB</td>
                            <td class="boolean">
                                {#if vmo.is_cow}
                                    <span class="badge cow-badge">YES</span>
                                {:else}
                                    <span class="badge std-badge">NO</span>
                                {/if}
                            </td>
                            <td class="parent">
                                {#if vmo.parent_handle !== 'None'}
                                    <span class="parent-link">↳ {vmo.parent_handle}</span>
                                {:else}
                                    <span style="color:#374151">-</span>
                                {/if}
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</div>

<style>
    .vmo-container { display:flex; flex-direction:column; height:100%; background:#020208; border:1px solid rgba(0,255,255,.1); font-family:'JetBrains Mono',monospace; font-size:10px; }
    .header { display:flex; justify-content:space-between; align-items:center; padding:10px 14px; background:#050510; border-bottom:1px solid rgba(0,255,255,.15); }
    .title { color:#22d3ee; font-weight:900; letter-spacing:.15em; font-size:11px; text-shadow:0 0 8px rgba(34,211,238,.3); }
    .active-count { color:#64748b; font-size:9px; font-weight:700; }
    .highlight { color:#34d399; text-shadow:0 0 5px #34d399; }
    .status-msg { padding:30px; text-align:center; letter-spacing:.1em; font-weight:700; }
    .table-wrap { flex:1; overflow-y:auto; }
    table { width:100%; border-collapse:collapse; text-align:left; }
    th { position:sticky; top:0; background:#0a0a1a; color:rgba(0,255,255,.4); padding:8px 12px; font-size:8px; letter-spacing:.1em; font-weight:900; border-bottom:1px solid rgba(0,255,255,.1); z-index:10; }
    td { padding:8px 12px; border-bottom:1px solid rgba(255,255,255,.03); color:#e2e8f0; }
    tr:hover td { background:rgba(0,255,255,.03); }
    tr.is-cow td { background:rgba(167,139,250,.02); }
    tr.is-cow:hover td { background:rgba(167,139,250,.06); }
    .handle { color:#22d3ee; font-weight:bold; }
    .numeric { color:#cbd5e1; }
    .ram-highlight { color:#fbbf24; font-weight:bold; text-shadow:0 0 4px rgba(251,191,36,.3); }
    .badge { padding:2px 6px; border-radius:3px; font-size:8px; font-weight:900; }
    .cow-badge { background:rgba(167,139,250,.15); color:#c084fc; border:1px solid rgba(167,139,250,.3); }
    .std-badge { background:rgba(52,211,153,.1); color:#34d399; border:1px solid rgba(52,211,153,.2); }
    .parent-link { color:#a78bfa; font-size:9px; font-style:italic; }
</style>
