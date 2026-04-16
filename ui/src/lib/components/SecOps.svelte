<script>
    import { pushTermLine } from '$lib/core/NexusWS.js';
    let scanning = $state(false);
    
    async function run(d) {
        scanning = true;
        pushTermLine(`>> [SECOPS] ${d}`, 'text-rose-400');
        try {
            const r = await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive: d })
            });
            if (r.ok) pushTermLine('>> Task queued — check terminal.', 'text-emerald-400');
            else pushTermLine(`!! HTTP ${r.status}`, 'text-rose-400');
        } finally {
            scanning = false;
        }
    }

    const OPS = [
        {l:'SECURITY AUDIT', d:'audit all rust source files for security vulnerabilities unsafe blocks and hardcoded secrets', c:'#f87171'},
        {l:'PATH TRAVERSAL', d:'check all file path handling code for directory traversal vulnerabilities', c:'#fb923c'},
        {l:'DEPENDENCY SCAN', d:'check Cargo.toml dependencies for known vulnerable versions', c:'#fbbf24'},
        {l:'SECRET SCANNER', d:'scan all source files for hardcoded secrets API keys or passwords', c:'#a78bfa'},
        {l:'FFI AUDIT', d:'audit all unsafe and extern C and Zig FFI bindings for memory safety issues', c:'#22d3ee'},
        {l:'SANDBOX VERIFY', d:'verify the sandbox path validation logic cannot be bypassed', c:'#34d399'},
    ];
</script>

<div style="display:flex;flex-direction:column;height:100%;background:#050000;font-family:'JetBrains Mono',monospace;">
    <div style="flex:none;display:flex;align-items:center;justify-content:space-between;padding:14px 16px;border-bottom:1px solid rgba(248,113,113,.12);background:#0a0000;">
        <div>
            <div style="font-size:11px;font-weight:900;letter-spacing:.15em;color:#f87171;text-shadow:0 0 12px rgba(248,113,113,.4);">SECOPS_CORTEX</div>
            <div style="font-size:8px;color:rgba(248,113,113,.35);margin-top:3px;">BIO-DOME SECURITY LAYER // ZIRCON_IPC</div>
        </div>
        <div style="font-size:32px;opacity:{scanning?1:.4};transition:opacity .3s;{scanning?'animation:spin 1s linear infinite':''}">🛡</div>
    </div>
    
    <div style="flex:1;overflow-y:auto;padding:14px;display:flex;flex-direction:column;gap:12px;">
        <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:6px;">
            {#each [['SANDBOX','ACTIVE','#34d399'],['ARBITER','ARMED','#34d399'],['LAZARUS','STANDBY','#fbbf24'],['MEMGUARD','ACTIVE','#34d399'],['FFI_GATE','LOCKED','#34d399'],['IPC_SHIELD','NOMINAL','#34d399']] as [l,s,c]}
                <div style="display:flex;align-items:center;gap:6px;padding:8px;border:1px solid rgba(248,113,113,.07);background:rgba(248,113,113,.02);">
                    <span style="width:6px;height:6px;border-radius:50%;background:{c};box-shadow:0 0 5px {c};flex:none;display:block;"></span>
                    <span style="font-size:8px;color:rgba(255,255,255,.3);flex:1;">{l}</span>
                    <span style="font-size:8px;font-weight:900;color:{c};">{s}</span>
                </div>
            {/each}
        </div>

        <div style="display:flex;flex-direction:column;gap:4px;">
            {#each OPS as op}
                <button 
                    onclick={()=>run(op.d)} 
                    disabled={scanning} 
                    style="display:flex;align-items:center;gap:10px;padding:10px 12px;border:1px solid rgba(255,255,255,.06);background:rgba(248,113,113,.02);cursor:pointer;transition:background .15s;text-align:left;width:100%;{scanning?'opacity:.5;cursor:not-allowed':''}" 
                    onmouseover={(e)=>!scanning&&(e.currentTarget.style.background='rgba(248,113,113,.06)')} 
                    onmouseout={(e)=>e.currentTarget.style.background='rgba(248,113,113,.02)'}
                    onfocus={(e)=>!scanning&&(e.currentTarget.style.background='rgba(248,113,113,.06)')}
                    onblur={(e)=>e.currentTarget.style.background='rgba(248,113,113,.02)'}
                >
                    <span style="flex:1;font-size:9px;font-weight:900;letter-spacing:.08em;color:{op.c};font-family:'JetBrains Mono',monospace;">{op.l}</span>
                    <span style="font-size:14px;color:rgba(255,255,255,.2);">›</span>
                </button>
            {/each}
        </div>

        <div style="display:flex;gap:8px;align-items:flex-start;padding:10px;border:1px solid rgba(248,113,113,.15);background:rgba(248,113,113,.04);font-size:9px;color:rgba(248,113,113,.6);line-height:1.5;">
            <span style="flex:none;color:#f87171;">⚠</span>
            <span>All sandbox escapes trigger immediate kernel-level thread termination via ZIRCON_IPC. Unauthorized access will be logged and reported.</span>
        </div>
    </div>
</div>

<style>
    @keyframes spin { to { transform: rotate(360deg); } }
</style>
