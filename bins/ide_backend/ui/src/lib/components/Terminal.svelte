<script lang="ts">
    import { TaskService } from '$lib/services/TaskService';
    import { terminal, clearGhost } from '$lib/stores/terminal.svelte';
    import { tick } from 'svelte';

    let logEl      = $state<HTMLDivElement | null>(null);
    let inputVal   = $state('');
    let autoScroll = $state(true);

    // Auto-scroll when logs or ghost buffer change
    $effect(() => {
        const _len = terminal.logs.length;
        const _buf = terminal.ghostBuffer;
        if (autoScroll && logEl) {
            tick().then(() => { if (logEl) logEl.scrollTop = logEl.scrollHeight; });
        }
    });

    async function sendDirective() {
        const d = inputVal.trim();
        if (!d) return;
        inputVal = '';
        TaskService.runDirective(d, 'USER_TERMINAL');
    }

    function onKey(e: KeyboardEvent) {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            sendDirective();
        }
    }

    function onScroll(e: Event) {
        const el = e.target as HTMLDivElement;
        // Re-enable auto-scroll if user scrolls back to bottom
        autoScroll = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    }

    function clearLogs() {
        // Use the store helper — direct array mutation breaks Svelte 5 reactivity
        terminal.logs = [];
        clearGhost();
    }
</script>

<div class="term-root">
    <div class="term-hdr">
        <span class="term-title">▶ NEURAL_OUTPUT</span>
        {#if terminal.isGhostTyping}
            <span class="typing-ind">
                <span class="td"></span><span class="td"></span><span class="td"></span>
                PROCESSING
            </span>
        {/if}
        <div class="term-actions">
            <button class="t-btn" onclick={clearLogs} title="Clear terminal">CLR</button>
            <button class="t-btn" onclick={() => autoScroll = !autoScroll}>
                {autoScroll ? '⤓ AUTO' : '⤒ LOCK'}
            </button>
        </div>
    </div>

    <div class="term-output" bind:this={logEl} onscroll={onScroll}>
        {#each terminal.logs as log (log.timestamp + log.message)}
            <div class="log-row {log.cls}">
                <span class="log-ts">{log.timestamp}</span>
                <span class="log-msg">{log.message}</span>
            </div>
        {/each}
        {#if terminal.ghostBuffer}
            <div class="ghost-out">
                <span class="ghost-lbl">GHOST ›</span>
                <span class="ghost-txt">{terminal.ghostBuffer}</span>
                {#if terminal.isGhostTyping}<span class="cursor">█</span>{/if}
            </div>
        {/if}
    </div>

    <div class="term-inp-row">
        <span class="prompt">❯</span>
        <input
            bind:value={inputVal}
            onkeydown={onKey}
            class="term-inp"
            placeholder="send directive..."
            autocomplete="off"
            spellcheck="false"
        />
        <button class="send-btn" onclick={sendDirective}>SEND</button>
    </div>
</div>

<style>
    .term-root { display:flex; flex-direction:column; height:100%; background:#000; font-family:'JetBrains Mono',monospace; }
    .term-hdr { flex:none; display:flex; align-items:center; height:26px; padding:0 10px; background:#010105; border-bottom:1px solid rgba(0,255,255,.05); gap:6px; }
    .term-title { font-size:8px; font-weight:900; color:#22d3ee; flex:1; }
    .typing-ind { display:flex; align-items:center; gap:4px; font-size:8px; color:#34d399; }
    .td { width:4px; height:4px; border-radius:50%; background:#34d399; display:inline-block; animation:td 1.2s infinite; }
    .td:nth-child(2){animation-delay:.2s;} .td:nth-child(3){animation-delay:.4s;}
    @keyframes td { 0%,80%,100%{opacity:.2;} 40%{opacity:1;} }
    .term-actions { display:flex; gap:4px; }
    .t-btn { background:transparent; border:1px solid rgba(0,255,255,.1); color:rgba(0,255,255,.3); font-size:8px; padding:2px 6px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:color .12s; }
    .t-btn:hover { color:#22d3ee; border-color:rgba(0,255,255,.4); }
    .term-output { flex:1; overflow-y:auto; padding:8px 10px; scrollbar-width:thin; scrollbar-color:#1e3a4a transparent; }
    .log-row { display:flex; gap:8px; font-size:10px; line-height:1.5; }
    .log-ts { color:rgba(255,255,255,.18); white-space:nowrap; flex:none; }
    .log-msg { flex:1; white-space:pre-wrap; word-break:break-word; }
    .ghost-out { margin-top:6px; padding:8px; border-left:2px solid #22d3ee; background:rgba(0,255,255,.02); }
    .ghost-lbl { font-size:8px; font-weight:900; color:#22d3ee; margin-right:6px; }
    .ghost-txt { font-size:10px; color:#c4b5fd; white-space:pre-wrap; }
    .cursor { color:#22d3ee; animation:blink .6s step-end infinite; }
    @keyframes blink { 50%{opacity:0;} }
    .term-inp-row { flex:none; display:flex; align-items:center; padding:6px 10px; background:#010105; border-top:1px solid rgba(0,255,255,.05); gap:6px; }
    .prompt { color:#22d3ee; font-size:12px; }
    .term-inp { flex:1; background:transparent; border:none; color:#e2e8f0; outline:none; font-size:11px; font-family:'JetBrains Mono',monospace; }
    .send-btn { background:rgba(34,211,238,.1); color:#22d3ee; border:1px solid rgba(0,255,255,.2); font-size:8px; padding:3px 10px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:background .12s; }
    .send-btn:hover { background:rgba(34,211,238,.2); }
</style>
