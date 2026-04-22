<script>
    // StatusBar.svelte — v1.2
    // FIX: 2s grace period before showing OFFLINE (matches NavHeader).
    // FIX: Temperature warning threshold 52°C → 58°C (more realistic).
    import { onMount, onDestroy } from 'svelte';
    import { wsState, ide, layout, systemVitals } from '$lib/core/store.svelte.js';

    let showStatus = $state(false); // grace gate
    let _graceTimer;

    onMount(() => { _graceTimer = setTimeout(() => showStatus = true, 2000); });
    onDestroy(() => clearTimeout(_graceTimer));

    let path      = $derived(ide.activeTab ?? '');
    let lang      = $derived.by(() => {
        const ext = path.split('.').at(-1)?.toLowerCase() ?? '';
        const m = { rs:'RUST', js:'JS', ts:'TS', svelte:'SVELTE', zig:'ZIG',
                    toml:'TOML', md:'MD', json:'JSON', sh:'BASH', css:'CSS', html:'HTML' };
        return m[ext] ?? 'TXT';
    });
    let isDirty   = $derived(ide.openTabs.find(t => t.path === path)?.isDirty ?? false);
    let tempColor = $derived(
        systemVitals.temperature > 68 ? '#f87171' :
        systemVitals.temperature > 58 ? '#fb923c' : '#34d399'  // FIX: 52→58
    );
    let isOnline  = $derived(showStatus ? wsState.ready : true); // show as online during grace
</script>

<footer class="status-bar">
    <div class="sb-left">
        <span class="sb-item" class:sb-live={isOnline} class:sb-dead={showStatus && !isOnline}>
            {!showStatus ? '● INIT' : isOnline ? '● ONLINE' : '● OFFLINE'}
        </span>
        <span class="sb-sep">│</span>
        <span class="sb-item">NEXUS v4.3.0</span>
        <span class="sb-sep">│</span>
        <span class="sb-item">ARM64 CROSTINI</span>
    </div>

    <div class="sb-center">
        {#if path}
            <span class="sb-path">{path}</span>
            <span class="sb-sep">·</span>
            <span class="sb-lang">{lang}</span>
            {#if isDirty}<span class="sb-dirty">● MODIFIED</span>{/if}
        {:else}
            <span class="sb-none">NO FILE ACTIVE</span>
        {/if}
    </div>

    <div class="sb-right">
        <span class="sb-item" style="color:{tempColor}">🌡 {(systemVitals.temperature||0).toFixed(1)}°C</span>
        <span class="sb-sep">│</span>
        <span class="sb-item" style="color:#22d3ee">⚡ {((systemVitals.cpu_load||0)*100).toFixed(0)}%</span>
        <span class="sb-sep">│</span>
        <span class="sb-item" style="color:#a78bfa">◈ {(systemVitals.ram_gb||0).toFixed(2)}GB</span>
        <span class="sb-sep">│</span>
        <button class="sb-btn" onclick={() => layout.explorerVisible = !layout.explorerVisible}>
            {layout.explorerVisible ? '◁ SIDEBAR' : '▷ SIDEBAR'}
        </button>
        <span class="sb-sep">│</span>
        <button class="sb-btn" onclick={() => layout.ghostVisible = !layout.ghostVisible}>
            {layout.ghostVisible ? '▷ GHOST' : '◁ GHOST'}
        </button>
    </div>
</footer>

<style>
    .status-bar { flex:none; height:22px; display:flex; align-items:center; background:#010108; border-top:1px solid rgba(0,255,255,.06); font-family:'JetBrains Mono',monospace; font-size:9px; padding:0 8px; user-select:none; gap:0; }
    .sb-left, .sb-right { display:flex; align-items:center; gap:5px; flex:none; }
    .sb-center { flex:1; display:flex; align-items:center; justify-content:center; gap:6px; }
    .sb-item { color:rgba(255,255,255,.22); white-space:nowrap; }
    .sb-live { color:#34d399 !important; }
    .sb-dead { color:#f87171 !important; }
    .sb-sep  { color:rgba(255,255,255,.08); }
    .sb-path { color:rgba(0,255,255,.4); overflow:hidden; text-overflow:ellipsis; max-width:360px; white-space:nowrap; }
    .sb-lang { font-weight:900; color:rgba(0,255,255,.3); }
    .sb-dirty { color:#fb923c; font-weight:700; font-size:8px; }
    .sb-none { color:rgba(255,255,255,.1); font-style:italic; }
    .sb-btn { background:none; border:none; cursor:pointer; color:rgba(255,255,255,.2); font-size:9px; font-family:'JetBrains Mono',monospace; padding:0; transition:color .12s; }
    .sb-btn:hover { color:rgba(0,255,255,.6); }
</style>
