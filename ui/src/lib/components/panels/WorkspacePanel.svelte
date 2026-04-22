<script>
    import { TaskService } from '$lib/services/TaskService.js';
    import { nexusSocket } from '$lib/nexus_socket.svelte';
    import Terminal from '$lib/components/Terminal.svelte';

    async function runDirective(directive) {
        TaskService.log(`>> [WORKSPACE] Queuing: ${directive}`, 'text-cyan-400');
        try {
            await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive })
            });
        } catch(e) {
            TaskService.log(`!! [FAULT] ${e.message}`, 'text-rose-400');
        }
    }

    function ghostEnshrine() {
        const handleEl = document.getElementById('ghost-handle');
        const textEl   = document.getElementById('ghost-text');
        const handle   = handleEl?.value.trim() ?? '';
        const text     = textEl?.value.trim()   || 'NEXUS_DATA';
        if (!handle) { alert('Enter a handle (e.g. 0xc8)'); return; }
        nexusSocket.send(`ghost enshrine ${handle} "${text}"`);
        TaskService.log(`>> [GHOST] enshrine ${handle} "${text}"`, 'text-purple-400');
    }

    function ghostScry() {
        const handleEl = document.getElementById('ghost-handle');
        const handle   = handleEl?.value.trim() ?? '';
        if (!handle) { alert('Enter a handle'); return; }
        nexusSocket.send(`ghost scry ${handle}`);
        TaskService.log(`>> [GHOST] scry ${handle}`, 'text-purple-400');
    }

    function ghostRecall() {
        const handleEl = document.getElementById('ghost-handle');
        const handle   = handleEl?.value.trim() ?? '';
        if (!handle) { alert('Enter a handle'); return; }
        nexusSocket.send(`ghost recall ${handle}`);
        TaskService.log(`>> [GHOST] recall ${handle}`, 'text-purple-400');
    }
</script>

<div class="panel-scroll ws-panel">
    <div class="ws-hero"><span class="ws-hero-title">WORKSPACE_DNA_MAP</span></div>
    <div class="ws-grid">
        <div class="ws-group">
            <div class="ws-group-title">BUILD_PIPELINE</div>
            <button class="ws-btn" onclick={() => runDirective('run cargo check and report all errors')}>▶ CARGO CHECK</button>
            <button class="ws-btn" onclick={() => runDirective('run npm run build in the ui directory')}>▶ UI BUILD</button>
            <button class="ws-btn" onclick={() => runDirective('run cargo build --release and report binary size')}>▶ RELEASE BUILD</button>
        </div>
        <div class="ws-group">
            <div class="ws-group-title">NEURAL_OPS</div>
            <button class="ws-btn" onclick={() => runDirective('audit all rust source files for security vulnerabilities')}>▶ SECURITY AUDIT</button>
            <button class="ws-btn" onclick={() => runDirective('find all TODO FIXME HACK and SUTURE comments in source')}>▶ SCAN TODOS</button>
            <button class="ws-btn" onclick={() => runDirective('run cargo clippy and list all warnings')}>▶ CLIPPY SCAN</button>
        </div>
        <div class="ws-group">
            <div class="ws-group-title">LAZARUS_OPS</div>
            <button class="ws-btn" onclick={() => runDirective('take a lazarus snapshot of src/main.rs tagged manual')}>▶ SNAPSHOT MAIN</button>
            <button class="ws-btn" onclick={() => runDirective('list all lazarus vault snapshots and their hashes')}>▶ LIST VAULT</button>
        </div>
        <div class="ws-group" style="grid-column: 1 / -1;">
            <div class="ws-group-title">GHOST_MEMORY_RITUAL</div>
            <div style="display:flex;flex-direction:column;gap:6px;font-size:9px;font-family:'JetBrains Mono',monospace;">
                <input id="ghost-handle" placeholder="VMO Handle e.g. 0xc8" class="ghost-ritual-inp" />
                <input id="ghost-text" placeholder="Text to enshrine (optional)" class="ghost-ritual-inp" />
                <div style="display:flex;gap:4px;">
                    <button onclick={ghostEnshrine} class="ws-btn" style="flex:1;">⬆ ENSHRINE</button>
                    <button onclick={ghostScry}    class="ws-btn" style="flex:1;">🔍 SCRY HEX</button>
                    <button onclick={ghostRecall}  class="ws-btn" style="flex:1;">📖 RECALL TEXT</button>
                </div>
            </div>
        </div>
    </div>
    <div class="ws-term" style="margin-top:16px;"><Terminal /></div>
</div>

<style>
    .panel-scroll { flex:1; overflow-y:auto; padding:18px; height:100%; display:flex; flex-direction:column; }
    .ws-hero-title { font-size:13px; font-weight:900; color:rgba(0,255,255,.4); letter-spacing:.2em; }
    .ws-grid { display:grid; grid-template-columns:1fr 1fr 1fr; gap:10px; margin-top:10px; }
    .ws-group { padding:12px; border:1px solid rgba(0,255,255,.06); background:rgba(0,255,255,.01); display:flex; flex-direction:column; gap:4px; }
    .ws-group-title { font-size:8px; font-weight:900; letter-spacing:.14em; color:rgba(0,255,255,.3); margin-bottom:6px; }
    .ws-btn { width:100%; text-align:left; padding:7px 10px; background:transparent; border:1px solid rgba(0,255,255,.1); color:#94a3b8; font-size:9px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .1s; }
    .ws-btn:hover { color:#00ffff; border-color:rgba(0,255,255,.4); background:rgba(0,255,255,.04); }
    .ghost-ritual-inp { background:#000; border:1px solid rgba(167,139,250,.3); color:#a78bfa; padding:6px 8px; outline:none; font-family:inherit; }
</style>
