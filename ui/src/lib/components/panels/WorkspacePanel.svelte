<script lang="ts">
    import { uplink } from '$lib/services/uplink.js';
    import { TaskService } from '$lib/services/TaskService';
    import Terminal from '$lib/components/Terminal.svelte';

    // FIX: Replaced DOM getElementById with bound Svelte state
    let ghostHandle = $state('');
    let ghostText   = $state('NEXUS_DATA');

    async function runDirective(directive: string) {
        TaskService.log(`>> [WORKSPACE] Queuing: ${directive}`, 'text-cyan-400');
        try {
            await uplink.post('/api/task', { directive });
        } catch (e: any) {
            TaskService.log(`!! [FAULT] ${e.message}`, 'text-rose-400');
        }
    }

    function ghostEnshrine() {
        const handle = ghostHandle.trim();
        const text   = ghostText.trim() || 'NEXUS_DATA';
        if (!handle) { TaskService.log('!! [GHOST] Handle required.', 'text-rose-400'); return; }
        runDirective(`ghost enshrine ${handle} "${text}"`);
        TaskService.log(`>> [GHOST] enshrine ${handle}`, 'text-purple-400');
    }

    function ghostScry() {
        const handle = ghostHandle.trim();
        if (!handle) { TaskService.log('!! [GHOST] Handle required.', 'text-rose-400'); return; }
        runDirective(`ghost scry ${handle}`);
    }

    function ghostRecall() {
        const handle = ghostHandle.trim();
        if (!handle) { TaskService.log('!! [GHOST] Handle required.', 'text-rose-400'); return; }
        runDirective(`ghost recall ${handle}`);
    }
</script>

<div class="panel-scroll ws-panel">
    <div class="ws-hero"><span class="ws-hero-title">WORKSPACE_DNA_MAP</span></div>
    <div class="ws-grid">
        <div class="ws-group">
            <div class="ws-group-title">BUILD_PIPELINE</div>
            <button class="ws-btn" onclick={() => runDirective('run cargo check')}>▶ CARGO CHECK</button>
            <button class="ws-btn" onclick={() => runDirective('run npm run build')}>▶ UI BUILD</button>
            <button class="ws-btn" onclick={() => runDirective('run cargo clippy')}>▶ CLIPPY</button>
        </div>
        <div class="ws-group">
            <div class="ws-group-title">GHOST_MEMORY</div>
            <input bind:value={ghostHandle} placeholder="Handle" class="ghost-ritual-inp" />
            <input bind:value={ghostText}   placeholder="Data"   class="ghost-ritual-inp" />
            <div style="display:flex; gap:4px;">
                <button onclick={ghostEnshrine} class="ws-btn">ENSHRINE</button>
                <button onclick={ghostScry}     class="ws-btn">SCRY</button>
                <button onclick={ghostRecall}   class="ws-btn">RECALL</button>
            </div>
        </div>
    </div>
    <div class="ws-term"><Terminal /></div>
</div>

<style>
    .panel-scroll { flex:1; overflow-y:auto; padding:18px; height:100%; display:flex; flex-direction:column; gap:10px; }
    .ws-hero-title { font-size:13px; font-weight:900; color:rgba(0,255,255,.4); letter-spacing:.2em; }
    .ws-grid { display:grid; grid-template-columns:1fr 1fr; gap:10px; }
    .ws-group { padding:12px; border:1px solid rgba(0,255,255,.06); background:rgba(0,255,255,.01); display:flex; flex-direction:column; gap:4px; }
    .ws-group-title { font-size:8px; font-weight:900; letter-spacing:.14em; color:rgba(0,255,255,.3); margin-bottom:4px; }
    .ws-btn { width:100%; text-align:left; padding:7px 10px; background:transparent; border:1px solid rgba(0,255,255,.1); color:#94a3b8; font-size:9px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .12s; }
    .ws-btn:hover { color:#22d3ee; border-color:rgba(0,255,255,.4); }
    .ghost-ritual-inp { background:#000; border:1px solid rgba(167,139,250,.3); color:#a78bfa; padding:6px 8px; outline:none; font-size:10px; font-family:'JetBrains Mono',monospace; width:100%; box-sizing:border-box; }
    .ghost-ritual-inp:focus { border-color:rgba(167,139,250,.6); }
    .ws-term { flex:1; min-height:200px; border:1px solid rgba(0,255,255,.06); overflow:hidden; }
</style>
