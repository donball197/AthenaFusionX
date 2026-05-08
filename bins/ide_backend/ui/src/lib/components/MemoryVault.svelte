<script>
    import { onMount } from 'svelte';
    import VmoTable from '$lib/components/VmoTable.svelte'; // ⚡ INJECTED VMO TABLE

    let memories = $state([]), loading = $state(true), searchQ = $state(''), purging = $state(false), msg = $state(''), expanded = $state(null);
    async function load() { loading=true; try { const r=await fetch('/api/memories'); if(r.ok) memories=await r.json(); } finally { loading=false; } }
    async function search() { if(!searchQ.trim()){load();return;} loading=true; try { const r=await fetch(`/api/memories/search?q=${encodeURIComponent(searchQ)}`); if(r.ok) memories=await r.json(); } finally { loading=false; } }
    async function purge() { if(!confirm('Purge ALL memory strands?')) return; purging=true; const r=await fetch('/api/memories/purge',{method:'DELETE'}); msg=r.ok?'Memory palace cleared.':'Purge failed.'; purging=false; await load(); setTimeout(()=>msg='',3000); }
    function ago(ts) { if(!ts) return ''; const d=new Date(ts), s=(Date.now()-d)/1000; return s<60?`${~~s}s ago`:s<3600?`${~~(s/60)}m ago`:`${~~(s/3600)}h ago`; }
    function oc(o) { const m={'NEXUS_ARCHITECT':'#22d3ee','ALPHA-SCOUT':'#34d399','SelfHealingEngine':'#f87171'}; return m[o]??'#a78bfa'; }
    onMount(load);
</script>
<div class="mv" style="display:flex;flex-direction:column;height:100%;background:#020008;font-family:'JetBrains Mono',monospace;">
    <div style="flex:none;display:flex;align-items:center;justify-content:space-between;padding:12px 16px;border-bottom:1px solid rgba(167,139,250,.1);background:#04000f;">
        <div style="font-size:11px;font-weight:900;letter-spacing:.15em;color:#a78bfa;display:flex;align-items:center;gap:6px;">◈ NEURAL_STRANDS</div>
        <div style="font-size:9px;color:rgba(167,139,250,.3);">{memories.length} STORED</div>
    </div>
    <div style="flex:none;padding:10px 12px;border-bottom:1px solid rgba(167,139,250,.06);">
        <div style="display:flex;gap:4px;margin-bottom:6px;">
            <input bind:value={searchQ} onkeydown={(e)=>e.key==='Enter'&&search()} placeholder="search memory..." style="flex:1;background:#000;border:1px solid rgba(167,139,250,.2);color:#c4b5fd;font-size:10px;padding:5px 8px;outline:none;font-family:'JetBrains Mono',monospace;" />
            <button onclick={search} style="background:rgba(167,139,250,.1);border:1px solid rgba(167,139,250,.2);color:#a78bfa;font-size:8px;font-weight:900;padding:0 10px;cursor:pointer;">SEARCH</button>
            <button onclick={load} style="background:rgba(167,139,250,.1);border:1px solid rgba(167,139,250,.2);color:#a78bfa;font-size:8px;font-weight:900;padding:0 10px;cursor:pointer;">ALL</button>
        </div>
        <div style="display:flex;align-items:center;gap:8px;">
            <button onclick={purge} disabled={purging} style="background:transparent;border:1px solid rgba(248,113,113,.2);color:rgba(248,113,113,.5);font-size:8px;font-weight:900;padding:3px 10px;cursor:pointer;font-family:'JetBrains Mono',monospace;">⚠ PURGE ALL</button>
            {#if msg}<span style="font-size:9px;color:#34d399;">{msg}</span>{/if}
        </div>
    </div>

    <div style="flex:1; padding:10px; min-height: 250px;">
        <VmoTable />
    </div>

    <div style="flex:1; overflow-y:auto; padding:8px; border-top: 1px solid rgba(167,139,250,.2);">
        <div style="font-size:8px;font-weight:900;letter-spacing:.15em;color:rgba(167,139,250,.4);margin-bottom:8px;padding-left:4px;">HISTORICAL_STRANDS</div>
        {#if loading}
            <div style="display:flex;flex-direction:column;align-items:center;gap:12px;padding:20px 0;color:rgba(167,139,250,.3);font-size:9px;letter-spacing:.1em;">
                <div style="width:28px;height:28px;border-radius:50%;border:2px solid rgba(167,139,250,.1);border-top-color:#a78bfa;animation:spin 1s linear infinite;"></div>
                SCANNING NEURAL ARCHIVE...
            </div>
        {:else if memories.length===0}
            <div style="padding:20px;text-align:center;font-size:9px;color:rgba(167,139,250,.2);letter-spacing:.15em;">NO MEMORY STRANDS FOUND</div>
        {:else}
            {#each memories as m}
                <div style="border:1px solid {expanded===m.id?'rgba(167,139,250,.3)':'rgba(167,139,250,.1)'};margin-bottom:5px;background:rgba(167,139,250,.02);transition:border-color .15s;">
                    <button onclick={()=>expanded=expanded===m.id?null:m.id} style="width:100%;background:none;border:none;cursor:pointer;padding:8px 10px;display:flex;align-items:center;gap:8px;text-align:left;">
                        <div style="display:flex;align-items:center;gap:6px;flex:none;">
                            <span style="font-size:8px;font-weight:900;color:{oc(m.origin)};">{m.origin}</span>
                            <span style="font-size:8px;color:rgba(167,139,250,.3);">#{m.id}</span>
                            <span style="font-size:8px;color:rgba(255,255,255,.2);">{ago(m.timestamp)}</span>
                        </div>
                        <div style="flex:1;font-size:10px;color:rgba(255,255,255,.6);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{m.directive}</div>
                        <span style="flex:none;font-size:8px;color:rgba(167,139,250,.4);">{expanded===m.id?'▲':'▼'}</span>
                    </button>
                    {#if expanded===m.id}
                        <div style="padding:10px;border-top:1px solid rgba(167,139,250,.08);background:#000;">
                            <div style="font-size:7px;font-weight:900;letter-spacing:.15em;color:rgba(167,139,250,.3);margin-bottom:5px;">RESULT</div>
                            <div style="font-size:10px;color:#94a3b8;white-space:pre-wrap;word-break:break-word;max-height:180px;overflow-y:auto;line-height:1.5;">{m.result}</div>
                            <div style="font-size:8px;color:rgba(255,255,255,.15);margin-top:8px;">{m.timestamp}</div>
                        </div>
                    {/if}
                </div>
            {/each}
        {/if}
    </div>
</div>
<style>@keyframes spin{to{transform:rotate(360deg);}}</style>
