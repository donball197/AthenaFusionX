<script>
import { terminal, clearGhost, finishGhost } from '$lib/stores/terminal.svelte';
import { layout, wsState } from '$lib/core/store.svelte';
import { TaskService } from '$lib/services/TaskService';

let directive = $state('');
let sending   = $state(false);

const QUICK = [
    { label: 'CARGO CHECK', d: 'run cargo check and show all errors' },
    { label: 'NPM BUILD',   d: 'run npm run build in the ui directory' },
    { label: 'AUDIT SRC',   d: 'audit all rust source files for security vulnerabilities' },
    { label: 'SCAN TODOS',  d: 'find all TODO FIXME HACK and SUTURE comments' },
    { label: 'LIST MEM',    d: 'list the 10 most recent memory strands' },
    { label: 'GIT STATUS',  d: 'run git status and show all changed files' },
];

async function fire() {
    const d = directive.trim();
    if (!d || sending) return;
    sending = true;
    clearGhost();
    directive = '';

    try {
        await TaskService.runDirective(d, 'GHOST_CORTEX');
    } finally {
        // finishGhost is called by runtime on TASK_STATUS, but guard here too
        sending = false;
    }
}

function onKey(e) {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); fire(); }
}
</script>

{#if layout.ghostVisible}
<aside class="ghost-blade">
    <div class="ghost-hdr">
        <div class="ghost-brand">
            <span class="ghost-icon">◈</span>
            <span class="ghost-name">GHOST_CORTEX</span>
        </div>
        <button class="ghost-close" onclick={() => layout.ghostVisible = false} aria-label="Close Ghost Cortex">×</button>
    </div>

    <!-- Quick commands -->
    <div class="quick-strip">
        {#each QUICK as q}
            <button class="quick-btn" onclick={() => { directive = q.d; fire(); }} title={q.d}>{q.label}</button>
        {/each}
    </div>

    <div class="stream-area">
        {#if terminal.ghostBuffer}
            <div class="stream-text">
                {terminal.ghostBuffer}
                {#if terminal.isGhostTyping}<span class="stream-cur">█</span>{/if}
            </div>
        {:else if terminal.isGhostTyping}
            <div class="thinking">
                <div class="think-dots"><span></span><span></span><span></span></div>
                <div class="think-lbl">NEURAL PROCESSING</div>
            </div>
        {:else}
            <div class="idle-state">
                <div class="idle-icon">◈</div>
                <div class="idle-title">GHOST CORTEX</div>
                <div class="idle-hint">AWAITING DIRECTIVE</div>
            </div>
        {/if}
    </div>

    <div class="section directive-section">
        <div class="section-hdr">
            <span>DIRECTIVE</span>
            {#if !wsState.ready}
                <span class="ws-warn">⚡ UPLINK SEVERED</span>
            {/if}
        </div>
        <textarea
            bind:value={directive}
            onkeydown={onKey}
            class="dir-textarea"
            placeholder="Enter directive...&#10;Enter to send, Shift+Enter for newline"
            rows="3"
            disabled={sending}
        ></textarea>
        <button class="fire-btn" class:fire-sending={sending} onclick={fire} disabled={sending}>
            {sending ? '⟳ PROCESSING...' : '▶ FIRE DIRECTIVE'}
        </button>
    </div>
</aside>
{:else}
    <button class="ghost-reopen" onclick={() => layout.ghostVisible = true} aria-label="Open Ghost Cortex">◈</button>
{/if}

<style>
    .ghost-blade { flex:none; width:240px; display:flex; flex-direction:column; background:#03000f; border-left:1px solid rgba(167,139,250,.12); overflow:hidden; }
    .ghost-hdr { flex:none; display:flex; align-items:center; height:38px; padding:0 10px; border-bottom:1px solid rgba(167,139,250,.1); background:#06001a; }
    .ghost-brand { flex:1; display:flex; align-items:center; gap:7px; }
    .ghost-icon { font-size:14px; color:#a78bfa; animation:ghost-glow 3s ease-in-out infinite; }
    @keyframes ghost-glow { 0%,100%{text-shadow:0 0 4px #a78bfa;opacity:.6;} 50%{text-shadow:0 0 14px #a78bfa;opacity:1;} }
    .ghost-name { font-size:9px; font-weight:900; letter-spacing:.12em; color:#a78bfa; }
    .ghost-close { background:none; border:none; color:rgba(167,139,250,.3); cursor:pointer; font-size:18px; line-height:1; padding:0 4px; transition:color .12s; }
    .ghost-close:hover { color:#a78bfa; }
    .quick-strip { flex:none; display:flex; flex-wrap:wrap; gap:3px; padding:6px 8px; border-bottom:1px solid rgba(167,139,250,.07); }
    .quick-btn { background:rgba(167,139,250,.06); border:1px solid rgba(167,139,250,.15); color:rgba(167,139,250,.5); font-size:7px; font-weight:900; padding:2px 5px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .12s; }
    .quick-btn:hover { background:rgba(167,139,250,.15); color:#a78bfa; }
    .stream-area { flex:1; min-height:80px; max-height:200px; overflow-y:auto; padding:10px; background:#000; scrollbar-width:thin; scrollbar-color:#4c1d95 transparent; }
    .stream-text { font-size:10.5px; color:#c4b5fd; line-height:1.6; white-space:pre-wrap; word-break:break-word; }
    .stream-cur { color:#a78bfa; animation:blink .6s step-end infinite; }
    @keyframes blink { 0%,100%{opacity:1;} 50%{opacity:0;} }
    .thinking { display:flex; flex-direction:column; align-items:center; gap:10px; padding:20px 0; }
    .think-dots { display:flex; gap:5px; }
    .think-dots span { width:6px; height:6px; border-radius:50%; background:#a78bfa; animation:td 1.2s infinite; }
    .think-dots span:nth-child(2){animation-delay:.2s;} .think-dots span:nth-child(3){animation-delay:.4s;}
    @keyframes td { 0%,80%,100%{opacity:.2;} 40%{opacity:1;} }
    .think-lbl { font-size:8px; color:rgba(167,139,250,.3); letter-spacing:.15em; }
    .idle-state { display:flex; flex-direction:column; align-items:center; gap:6px; padding:20px 0; }
    .idle-icon { font-size:28px; color:rgba(167,139,250,.2); }
    .idle-title { font-size:9px; font-weight:900; color:rgba(167,139,250,.3); letter-spacing:.1em; }
    .idle-hint { font-size:8px; color:rgba(167,139,250,.2); }
    .section { flex:none; padding:8px 10px; border-top:1px solid rgba(167,139,250,.07); }
    .directive-section { display:flex; flex-direction:column; gap:6px; }
    .section-hdr { display:flex; justify-content:space-between; align-items:center; font-size:7px; font-weight:900; letter-spacing:.14em; color:rgba(167,139,250,.3); margin-bottom:4px; }
    .ws-warn { color:#fb923c; font-size:7px; animation:pulse 1.5s infinite; }
    @keyframes pulse { 50%{opacity:.3;} }
    .dir-textarea { width:100%; background:#000; border:1px solid rgba(167,139,250,.2); color:#e2e8f0; font-family:'JetBrains Mono',monospace; font-size:10px; padding:6px 8px; resize:none; outline:none; box-sizing:border-box; caret-color:#a78bfa; line-height:1.5; transition:border-color .12s; }
    .dir-textarea:focus { border-color:rgba(167,139,250,.5); }
    .fire-btn { width:100%; padding:8px; background:rgba(167,139,250,.12); border:1px solid rgba(167,139,250,.3); color:#a78bfa; font-size:9px; font-weight:900; letter-spacing:.1em; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .15s; }
    .fire-btn:hover:not(:disabled) { background:rgba(167,139,250,.22); }
    .fire-btn.fire-sending { animation:fire-pulse .8s ease-in-out infinite alternate; cursor:not-allowed; }
    @keyframes fire-pulse { from{opacity:.5;} to{opacity:1;} }
    .ghost-reopen { position:fixed; right:0; top:50%; transform:translateY(-50%); background:rgba(167,139,250,.1); border:1px solid rgba(167,139,250,.2); border-right:none; color:#a78bfa; padding:14px 5px; cursor:pointer; font-size:14px; z-index:100; }
</style>
