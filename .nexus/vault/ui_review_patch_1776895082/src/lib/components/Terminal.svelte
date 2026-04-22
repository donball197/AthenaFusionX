<script>
    import { uplink } from "$lib/services/uplink.js";
    // ════════════════════════════════════════════════════════════
    //  Terminal — Live log feed + directive input
    // ════════════════════════════════════════════════════════════
    import { terminalState } from '$lib/core/store.svelte.js';
    import { tick } from 'svelte';

    let logEl      = $state(null);
    let inputVal   = $state('');
    let autoScroll = $state(true);

    $effect(() => {
        const _len = terminalState.logs.length;
        const _buf = terminalState.buffers.GHOST_CORTEX;
        if (autoScroll && logEl) {
            tick().then(() => { if (logEl) logEl.scrollTop = logEl.scrollHeight; });
        }
    });

    async function sendDirective() {
        const d = inputVal.trim();
        if (!d) return;
        inputVal = '';
        try {
            await uplink.fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive: d })
            });
        } catch(e) {}
    }

    function onKey(e) {
        if (e.key === 'Enter') sendDirective();
    }

    function onScroll(e) {
        const el = e.target;
        autoScroll = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    }

    function clearLogs() {
        terminalState.logs = [];
        terminalState.buffers.GHOST_CORTEX = '';
        terminalState.isGhostTyping = false;
    }
</script>

<div class="term-root">
    <!-- Header -->
    <div class="term-hdr">
        <span class="term-title">▶ NEURAL_OUTPUT</span>
        {#if terminalState.isGhostTyping}
            <span class="typing-ind">
                <span class="td"></span><span class="td"></span><span class="td"></span>
                PROCESSING
            </span>
        {/if}
        <div class="term-actions">
            <button class="t-btn" onclick={clearLogs}>CLR</button>
            <button class="t-btn" onclick={() => autoScroll = !autoScroll} title="Auto-scroll">
                {autoScroll ? '⤓ AUTO' : '⤒ LOCK'}
            </button>
        </div>
    </div>

    <!-- Log output -->
    <div class="term-output" bind:this={logEl} onscroll={onScroll}>
        {#each terminalState.logs as log}
            <div class="log-row {log.cls}">
                <span class="log-ts">{log.timestamp}</span>
                <span class="log-msg">{log.message}</span>
            </div>
        {/each}

        {#if terminalState.buffers?.GHOST_CORTEX}
            <div class="ghost-out">
                <span class="ghost-lbl">GHOST ›</span>
                <span class="ghost-txt">{terminalState.buffers.GHOST_CORTEX}</span>
                {#if terminalState.isGhostTyping}
                    <span class="cursor">█</span>
                {/if}
            </div>
        {/if}

        {#if terminalState.logs.length === 0 && !terminalState.buffers?.GHOST_CORTEX}
            <div class="term-idle">── NEXUS TERMINAL READY ── type a directive below ──</div>
        {/if}
    </div>

    <!-- Input row -->
    <div class="term-inp-row">
        <span class="prompt">❯</span>
        <input
            bind:value={inputVal}
            onkeydown={onKey}
            class="term-inp"
            placeholder="send directive to NEXUS..."
            autocomplete="off"
            spellcheck="false"
        />
        <button class="send-btn" onclick={sendDirective}>SEND</button>
    </div>
</div>

<style>
    .term-root { display:flex; flex-direction:column; height:100%; background:#000; font-family:'JetBrains Mono',monospace; }

    .term-hdr {
        flex:none; display:flex; align-items:center; gap:8px;
        height:26px; padding:0 10px;
        background:#010105; border-bottom:1px solid rgba(0,255,255,.05);
    }
    .term-title { font-size:8px; font-weight:900; letter-spacing:.12em; color:#22d3ee; flex:1; }
    .typing-ind { display:flex; align-items:center; gap:3px; font-size:8px; color:#34d399; letter-spacing:.08em; }
    .td { width:4px; height:4px; border-radius:50%; background:#34d399; animation:td-pulse 1.2s infinite; }
    .td:nth-child(2) { animation-delay:.2s; }
    .td:nth-child(3) { animation-delay:.4s; }
    @keyframes td-pulse { 0%,80%,100%{opacity:.2;} 40%{opacity:1;} }
    .term-actions { display:flex; gap:4px; }
    .t-btn { background:transparent; border:1px solid rgba(0,255,255,.1); color:rgba(0,255,255,.3); font-size:8px; padding:2px 7px; cursor:pointer; font-family:'JetBrains Mono',monospace; transition:all .12s; }
    .t-btn:hover { color:#22d3ee; border-color:rgba(0,255,255,.3); }

    .term-output { flex:1; overflow-y:auto; padding:8px 10px; }
    .term-output::-webkit-scrollbar { width:3px; }
    .term-output::-webkit-scrollbar-thumb { background:#1e3a4a; }

    .log-row { display:flex; gap:8px; padding:1px 0; font-size:10px; line-height:1.5; }
    .log-ts { flex:none; color:rgba(255,255,255,.18); font-size:9px; }
    .log-msg { flex:1; word-break:break-word; white-space:pre-wrap; }

    /* Tailwind color classes used by pushTermLine */
    :global(.text-gray-400)    { color:#9ca3af; }
    :global(.text-emerald-500) { color:#10b981; }
    :global(.text-emerald-400) { color:#34d399; }
    :global(.text-rose-500)    { color:#ef4444; }
    :global(.text-rose-400)    { color:#f87171; }
    :global(.text-sky-400)     { color:#38bdf8; }
    :global(.text-cyan-400)    { color:#22d3ee; }
    :global(.text-purple-400)  { color:#c084fc; }
    :global(.text-amber-400)   { color:#fbbf24; }

    .ghost-out {
        margin-top:6px; padding:8px 10px;
        border:1px solid rgba(0,255,255,.1); border-left:2px solid #22d3ee;
        background:rgba(0,255,255,.02);
    }
    .ghost-lbl { font-size:8px; font-weight:900; color:#22d3ee; margin-right:6px; }
    .ghost-txt { font-size:11px; color:#94a3b8; white-space:pre-wrap; }
    .cursor { color:#22d3ee; animation:blink .6s step-end infinite; }
    @keyframes blink { 0%,100%{opacity:1;} 50%{opacity:0;} }

    .term-idle { font-size:9px; color:rgba(0,255,255,.12); padding:14px 0; text-align:center; letter-spacing:.08em; }

    .term-inp-row {
        flex:none; display:flex; align-items:center; gap:6px;
        padding:6px 10px; border-top:1px solid rgba(0,255,255,.05);
        background:#010105;
    }
    .prompt { color:#22d3ee; font-size:13px; flex:none; }
    .term-inp {
        flex:1; background:transparent; border:none; outline:none;
        color:#e2e8f0; font-family:'JetBrains Mono',monospace; font-size:11px;
        caret-color:#00ffff;
    }
    .term-inp::placeholder { color:rgba(0,255,255,.15); }
    .send-btn {
        background:rgba(34,211,238,.1); border:1px solid rgba(0,255,255,.2);
        color:#22d3ee; font-size:8px; font-weight:900; letter-spacing:.1em;
        padding:3px 10px; cursor:pointer; font-family:'JetBrains Mono',monospace;
        transition:background .12s;
    }
    .send-btn:hover { background:rgba(34,211,238,.22); }
</style>
