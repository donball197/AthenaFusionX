<script>
    // NavHeader.svelte — v1.1
    // FIX: clock setInterval leaked — now cleared in onDestroy.
    // FIX: wsState.ready replaced with wsReady $derived from store.
    import { onMount, onDestroy } from 'svelte';
    import { layout, wsReady, systemVitals } from '$lib/core/store.svelte.js';

    let { activeNav = $bindable('WORKSPACE') } = $props();

    const TABS = [
        { id: 'NEXUS_OS',       color: '#00ffff' },
        { id: 'IDE',            color: '#22d3ee' },
        { id: 'WORKSPACE',      color: '#34d399' },
        { id: 'VITALS',         color: '#fb923c' },
        { id: 'MEMORY',         color: '#a78bfa' },
        { id: 'SECOPS',         color: '#f87171' },
        { id: 'AGENT-LAB',      color: '#fbbf24' },
        { id: 'HISTORY',        color: '#818cf8' },
        { id: 'DYNAMIC_UI',     color: '#e879f9' },
        { id: 'COGNITIVE_MESH', color: '#f472b6' },
    ];

    let clockTime = $state('00:00:00');
    let _clockInterval;

    onMount(() => {
        clockTime = new Date().toLocaleTimeString('en-US', { hour12: false });
        _clockInterval = setInterval(() => {
            clockTime = new Date().toLocaleTimeString('en-US', { hour12: false });
        }, 1000);
    });

    // FIX: clear interval on destroy to prevent memory leak
    onDestroy(() => clearInterval(_clockInterval));

    let tempColor = $derived(
        systemVitals.temperature > 70 ? '#f87171'  :
        systemVitals.temperature > 55 ? '#fb923c'  : '#34d399'
    );
</script>

<header class="nexus-nav">
    <div class="nav-left">
        <button class="hamburger"
            onclick={() => layout.explorerVisible = !layout.explorerVisible}
            title="Toggle sidebar"
            aria-label="Toggle sidebar">
            <span></span><span></span><span></span>
        </button>
        <div class="brand">
            <span class="brand-n">NEXUS</span>
            <span class="brand-s">://</span>
            <span class="brand-v">SINGULARITY</span>
            <!-- FIX: uses wsReady $derived instead of wsState.ready getter -->
            <span class="brand-dot" class:dot-live={wsReady}></span>
        </div>
    </div>

    <nav class="tab-strip" aria-label="Main navigation">
        {#each TABS as t}
            <button class="nav-tab"
                class:is-active={activeNav === t.id}
                style="--c:{t.color}"
                onclick={() => activeNav = t.id}
                aria-current={activeNav === t.id ? 'page' : undefined}>
                {t.id}
            </button>
        {/each}
    </nav>

    <div class="nav-right">
        <span class="metric" style="color:{tempColor}" title="Temperature">
            🌡 {(systemVitals.temperature || 0).toFixed(1)}°C
        </span>
        <span class="metric-sep">│</span>
        <span class="metric" style="color:#22d3ee" title="CPU Load">
            ⚡ {((systemVitals.cpu_load || 0) * 100).toFixed(0)}%
        </span>
        <span class="metric-sep">│</span>
        <span class="metric" style="color:#a78bfa" title="RAM Used">
            ◈ {(systemVitals.ram_gb || 0).toFixed(2)}GB
        </span>
        <span class="metric-sep">│</span>
        <span class="clock">{clockTime}</span>
        <!-- FIX: uses wsReady $derived -->
        <span class="ws-pill" class:ws-live={wsReady}>
            {wsReady ? '● LIVE' : '○ OFFLINE'}
        </span>
        <button class="ghost-toggle"
            onclick={() => layout.ghostVisible = !layout.ghostVisible}
            title="Toggle Ghost Cortex"
            aria-label="Toggle Ghost Cortex">◈</button>
    </div>
</header>

<style>
    .nexus-nav {
        flex: none; display: flex; align-items: center;
        height: 42px; background: #02020c;
        border-bottom: 1px solid rgba(0,255,255,.1);
        padding: 0 10px; gap: 8px; user-select: none;
        position: relative; z-index: 200;
        background-image: repeating-linear-gradient(
            0deg, transparent, transparent 3px,
            rgba(0,255,255,.008) 3px, rgba(0,255,255,.008) 6px
        );
    }
    .nav-left { display:flex; align-items:center; gap:10px; flex:none; min-width:180px; }
    .hamburger {
        display:flex; flex-direction:column; gap:4px;
        background:none; border:none; cursor:pointer; padding:4px;
        opacity:.5; transition:opacity .15s;
    }
    .hamburger:hover { opacity:1; }
    .hamburger span { display:block; width:18px; height:1.5px; background:#22d3ee; border-radius:1px; }
    .brand { display:flex; align-items:center; gap:4px; }
    .brand-n { font-size:12px; font-weight:900; font-style:italic; letter-spacing:.15em; color:#00ffff; text-shadow:0 0 14px rgba(0,255,255,.6); }
    .brand-s { font-size:10px; color:rgba(0,255,255,.3); }
    .brand-v { font-size:9px; color:rgba(0,255,255,.35); letter-spacing:.08em; }
    .brand-dot { width:6px; height:6px; border-radius:50%; background:#374151; margin-left:4px; transition:all .4s; }
    .brand-dot.dot-live { background:#34d399; box-shadow:0 0 6px #34d399,0 0 14px rgba(52,211,153,.4); animation:dot-breathe 2.5s ease-in-out infinite; }
    @keyframes dot-breathe { 0%,100%{box-shadow:0 0 4px #34d399;} 50%{box-shadow:0 0 12px #34d399,0 0 28px rgba(52,211,153,.5);} }
    .tab-strip { flex:1; display:flex; align-items:stretch; justify-content:flex-start; height:100%; overflow-x:auto; overflow-y:hidden; scrollbar-width:none; }
    .tab-strip::-webkit-scrollbar { display:none; }
    .nav-tab { flex:none; padding:0 11px; background:transparent; border:none; border-right:1px solid rgba(0,255,255,.04); font-size:9px; font-weight:900; font-family:'JetBrains Mono',monospace; letter-spacing:.1em; color:rgba(255,255,255,.2); cursor:pointer; transition:all .12s; position:relative; white-space:nowrap; }
    .nav-tab:hover { color:var(--c); background:rgba(0,255,255,.03); }
    .nav-tab.is-active { color:var(--c); text-shadow:0 0 10px var(--c); background:rgba(0,0,0,.4); }
    .nav-tab.is-active::after { content:''; position:absolute; bottom:0; left:0; right:0; height:2px; background:var(--c); box-shadow:0 0 8px var(--c); }
    .nav-right { display:flex; align-items:center; gap:7px; flex:none; min-width:330px; justify-content:flex-end; }
    .metric { font-size:9px; font-weight:700; font-family:'JetBrains Mono',monospace; white-space:nowrap; }
    .metric-sep { color:rgba(255,255,255,.1); font-size:9px; }
    .clock { font-size:9px; color:rgba(255,255,255,.25); font-family:'JetBrains Mono',monospace; }
    .ws-pill { font-size:8px; font-weight:900; letter-spacing:.06em; padding:2px 7px; border:1px solid rgba(55,65,81,.5); color:#374151; transition:all .3s; }
    .ws-pill.ws-live { color:#34d399; border-color:rgba(52,211,153,.4); text-shadow:0 0 6px rgba(52,211,153,.5); }
    .ghost-toggle { background:rgba(167,139,250,.1); border:1px solid rgba(167,139,250,.2); color:#a78bfa; font-size:13px; width:26px; height:26px; cursor:pointer; transition:all .15s; display:flex; align-items:center; justify-content:center; }
    .ghost-toggle:hover { background:rgba(167,139,250,.2); box-shadow:0 0 8px rgba(167,139,250,.3); }
</style>
