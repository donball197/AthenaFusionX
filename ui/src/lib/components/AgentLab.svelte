<script>
    import { pushTermLine } from '$lib/core/NexusWS.js';
    let missions=$state([]),loading=$state(false),directive=$state('');
    const AGENTS=Array.from({length:12},(_,i)=>({id:i,name:`DRONE_${String(i).padStart(2,'0')}`,status:i<3?'ACTIVE':i<6?'STANDBY':'IDLE',task:i<3?['SCANNING','HEALING','AUDITING'][i]:null}));
    const SC={ACTIVE:'#34d399',STANDBY:'#fbbf24',IDLE:'#374151'};
    async function loadMissions(){loading=true;try{const r=await fetch('/api/missions');if(r.ok)missions=await r.json();}finally{loading=false;}}
    async function broadcast(){const d=directive.trim();if(!d)return;directive='';pushTermLine(`>> [SWARM] Broadcasting: ${d}`,'text-amber-400');try{await fetch('/api/task',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({directive:`[SWARM_TASK] ${d}`})});}catch(e){}}
    loadMissions();
</script>
<div style="display:flex;flex-direction:column;height:100%;background:#02030a;font-family:'JetBrains Mono',monospace;">
    <div style="flex:none;display:flex;align-items:center;justify-content:space-between;padding:14px 16px;border-bottom:1px solid rgba(251,191,36,.1);background:#03040f;">
        <div>
            <div style="font-size:11px;font-weight:900;letter-spacing:.15em;color:#fbbf24;text-shadow:0 0 10px rgba(251,191,36,.4);">SWARM_REFLEX_LAB</div>
            <div style="font-size:8px;color:rgba(251,191,36,.3);margin-top:3px;">MULTI-AGENT ORCHESTRATION // NODE_COUNT: {AGENTS.length}</div>
        </div>
        <button onclick={loadMissions} style="background:rgba(251,191,36,.1);border:1px solid rgba(251,191,36,.2);color:#fbbf24;font-size:14px;padding:4px 10px;cursor:pointer;">⟳</button>
    </div>
    <div style="flex:1;overflow-y:auto;padding:14px;display:flex;flex-direction:column;gap:12px;">
        <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:6px;">
            {#each AGENTS as a}
                <div style="display:flex;align-items:center;gap:8px;padding:8px;border:1px solid {SC[a.status]}22;background:rgba(0,0,0,.3);">
                    <span style="width:8px;height:8px;border-radius:50%;background:{SC[a.status]};box-shadow:0 0 6px {SC[a.status]};flex:none;display:block;"></span>
                    <div style="min-width:0;">
                        <div style="font-size:8px;font-weight:900;color:rgba(255,255,255,.5);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{a.name}</div>
                        <div style="font-size:7px;font-weight:700;color:{SC[a.status]};">{a.status}</div>
                        {#if a.task}<div style="font-size:7px;color:rgba(251,191,36,.5);">{a.task}</div>{/if}
                    </div>
                </div>
            {/each}
        </div>
        <div style="border:1px solid rgba(251,191,36,.1);padding:10px;background:rgba(251,191,36,.02);">
            <div style="font-size:8px;font-weight:900;letter-spacing:.15em;color:rgba(251,191,36,.4);margin-bottom:8px;">ACTIVE_MISSIONS</div>
            {#if loading}<div style="font-size:9px;color:rgba(255,255,255,.2);text-align:center;padding:12px;">SCANNING...</div>
            {:else if missions.length===0}<div style="font-size:9px;color:rgba(255,255,255,.15);text-align:center;padding:12px;">NO ACTIVE MISSIONS</div>
            {:else}
                {#each missions as m}
                    <div style="display:flex;align-items:center;gap:8px;padding:6px;border-bottom:1px solid rgba(251,191,36,.06);font-size:9px;">
                        <span style="color:rgba(251,191,36,.4);flex:none;">{m.id?.slice(0,8)}</span>
                        <span style="flex:1;color:rgba(255,255,255,.5);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{m.goal}</span>
                        <span style="flex:none;color:#34d399;font-size:8px;">{m.status}</span>
                    </div>
                {/each}
            {/if}
        </div>
        <div style="border:1px solid rgba(251,191,36,.1);padding:10px;">
            <div style="font-size:8px;font-weight:900;letter-spacing:.15em;color:rgba(251,191,36,.4);margin-bottom:6px;">BROADCAST TO SWARM</div>
            <div style="display:flex;gap:4px;">
                <input bind:value={directive} onkeydown={(e)=>e.key==='Enter'&&broadcast()} placeholder="broadcast directive to all drones..." style="flex:1;background:#000;border:1px solid rgba(251,191,36,.2);color:#fbbf24;font-size:10px;padding:6px 8px;outline:none;font-family:'JetBrains Mono',monospace;" />
                <button onclick={broadcast} style="background:rgba(251,191,36,.1);border:1px solid rgba(251,191,36,.3);color:#fbbf24;font-size:8px;font-weight:900;padding:0 12px;cursor:pointer;font-family:'JetBrains Mono',monospace;">BROADCAST</button>
            </div>
        </div>
    </div>
</div>
