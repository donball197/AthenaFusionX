<script>
    // VitalsView.svelte — v2.2
    // FIX: HTTP poll interval raised to 5000ms — WS HEARTBEAT at 800ms
    //      already keeps systemVitals fresh. HTTP poll is just a fallback.
    // FIX: Single interval drives both fetch and history push to halve
    //      the timer count on this constrained device.
    // FIX: ramMax $derived now uses Math.max guard against 0.
    import { onMount, onDestroy } from 'svelte';
    import { systemVitals } from '$lib/core/store.svelte.js';

    const MAX = 40;
    let cpuHist  = $state([]);
    let tempHist = $state([]);
    let ramHist  = $state([]);
    let interval;

    async function tick() {
        // Push history first (always)
        cpuHist  = [...cpuHist.slice(-(MAX-1)),  (systemVitals.cpu_load  || 0) * 100];
        tempHist = [...tempHist.slice(-(MAX-1)),  systemVitals.temperature || 0];
        ramHist  = [...ramHist.slice(-(MAX-1)),   systemVitals.ram_gb      || 0];

        // HTTP fallback — only if WS might be stale (temperature === 0 suggests no heartbeat)
        if (systemVitals.temperature === 0) {
            try {
                const r = await fetch('/api/vitals');
                if (r.ok) {
                    const d = await r.json();
                    systemVitals.cpu_load     = d.cpu_load     ?? systemVitals.cpu_load;
                    systemVitals.ram_gb       = d.ram_gb       ?? systemVitals.ram_gb;
                    systemVitals.total_ram_gb = d.total_ram_gb ?? systemVitals.total_ram_gb;
                    systemVitals.temperature  = d.temperature  ?? systemVitals.temperature;
                }
            } catch (e) {
                console.warn('[VitalsView] HTTP fallback fault:', e.message);
            }
        }
    }

    onMount(() => { tick(); interval = setInterval(tick, 5000); });
    onDestroy(() => clearInterval(interval));

    let tc = $derived(
        systemVitals.temperature > 70 ? '#f87171' :
        systemVitals.temperature > 55 ? '#fb923c' : '#34d399'
    );
    let cc = $derived(
        systemVitals.cpu_load > 0.8 ? '#f87171' :
        systemVitals.cpu_load > 0.5 ? '#fbbf24' : '#22d3ee'
    );
    let ramMax = $derived(Math.max(systemVitals.total_ram_gb || 4, ...ramHist, 0.1));

    function sp(data, w, h, max) {
        if (data.length < 2) return '';
        return 'M' + data.map((v, i) =>
            `${(i/(data.length-1))*w},${h-(v/max)*h}`
        ).join('L');
    }
    function arc(pct, r) {
        const a = Math.min(Math.max(pct, 0), 0.9999) * Math.PI;
        const x = 60 + r * Math.cos(Math.PI + a);
        const y = 60 + r * Math.sin(Math.PI + a);
        return `M ${60-r} 60 A ${r} ${r} 0 ${a>Math.PI?1:0} 1 ${x} ${y}`;
    }
</script>

<div style="display:flex;flex-direction:column;height:100%;padding:16px;gap:14px;background:#000;overflow-y:auto;font-family:'JetBrains Mono',monospace;">

    <div>
        <div style="font-size:12px;font-weight:900;letter-spacing:.2em;color:#22d3ee;text-shadow:0 0 12px rgba(34,211,238,.4);">SYSTEM_VITALS</div>
        <div style="font-size:8px;color:rgba(0,255,255,.25);margin-top:2px;">REAL-TIME HARDWARE TELEMETRY // ARM64 CROSTINI</div>
    </div>

    <div style="display:flex;gap:10px;">
        {#each [
            {val:systemVitals.cpu_load||0, max:1,      label:'CPU LOAD',  fmt:v=>`${(v*100).toFixed(0)}%`,  c:cc},
            {val:systemVitals.temperature||0, max:100, label:'THERMAL',   fmt:v=>`${v.toFixed(1)}°C`,        c:tc},
            {val:systemVitals.ram_gb||0,   max:ramMax, label:'RAM USED',  fmt:v=>`${v.toFixed(1)}G`,         c:'#a78bfa'},
        ] as g}
            <div style="flex:1;display:flex;flex-direction:column;align-items:center;gap:4px;">
                <svg viewBox="0 0 120 70" style="width:100%;max-width:120px;">
                    <path d={arc(1,48)} fill="none" stroke="rgba(255,255,255,.05)" stroke-width="8" stroke-linecap="round"/>
                    <path d={arc(g.val/g.max,48)} fill="none" stroke={g.c} stroke-width="8" stroke-linecap="round"
                          style="filter:drop-shadow(0 0 4px {g.c})"/>
                    <text x="60" y="60" text-anchor="middle" fill={g.c}
                          font-family="JetBrains Mono,monospace" font-size="13" font-weight="900">{g.fmt(g.val)}</text>
                </svg>
                <div style="font-size:8px;font-weight:900;letter-spacing:.12em;color:rgba(255,255,255,.3);">{g.label}</div>
            </div>
        {/each}
    </div>

    {#each [
        {data:cpuHist,  max:100,    label:'CPU HISTORY',     c:cc},
        {data:tempHist, max:100,    label:'THERMAL HISTORY', c:tc},
        {data:ramHist,  max:ramMax, label:'RAM HISTORY',     c:'#a78bfa'},
    ] as s}
        <div style="border:1px solid rgba(0,255,255,.06);padding:8px;background:rgba(0,0,0,.5);">
            <div style="font-size:8px;font-weight:900;letter-spacing:.1em;color:{s.c};margin-bottom:5px;">{s.label}</div>
            <svg viewBox="0 0 160 40" style="width:100%;height:40px;display:block;" preserveAspectRatio="none">
                {#if s.data.length >= 2}
                    <path d="{sp(s.data,160,40,s.max)} L160,40 L0,40 Z" fill={s.c} opacity=".2"/>
                    <path d={sp(s.data,160,40,s.max)} fill="none" stroke={s.c} stroke-width="1.5"
                          style="filter:drop-shadow(0 0 3px {s.c})"/>
                {:else}
                    <text x="80" y="25" text-anchor="middle" fill="rgba(255,255,255,.15)"
                          font-size="9" font-family="monospace">collecting data…</text>
                {/if}
            </svg>
        </div>
    {/each}

    <div style="border:1px solid rgba(0,255,255,.06);padding:10px;background:rgba(0,0,0,.5);">
        <div style="font-size:8px;font-weight:900;letter-spacing:.15em;color:rgba(0,255,255,.3);margin-bottom:6px;">RAW TELEMETRY</div>
        <pre style="margin:0;font-size:10px;color:#64748b;white-space:pre;">{JSON.stringify({
  cpu:  `${((systemVitals.cpu_load||0)*100).toFixed(2)}%`,
  temp: `${(systemVitals.temperature||0).toFixed(2)}°C`,
  ram:  `${(systemVitals.ram_gb||0).toFixed(3)} / ${(systemVitals.total_ram_gb||4).toFixed(1)}GB`,
  res:  (systemVitals.resonance||0).toFixed(4),
},null,2)}</pre>
    </div>
</div>
