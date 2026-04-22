<script>
    // CommandPalette.svelte — v2.0
    // FIX: Commands now actually fire as directives via /api/task.
    // FIX: Query field filters commands in real-time.
    import { onMount } from 'svelte';
    import { fade, scale } from 'svelte/transition';
    import SoundFx from '$lib/SoundFx.js';
    import { TaskService } from '$lib/services/TaskService.js';

    let visible       = $state(false);
    let query         = $state('');
    let selectedIndex = $state(0);

    const ALL_COMMANDS = [
        { id:'audit',   name:'RUN_SECURITY_AUDIT',     icon:'🛡️', desc:'Scan workspace for vulnerabilities',    directive:'audit all rust source files for security vulnerabilities unsafe blocks and hardcoded secrets' },
        { id:'build',   name:'TRIGGER_RELEASE_BUILD',  icon:'📦', desc:'Compile optimized ARM64 binary',        directive:'run cargo build --release and report binary size' },
        { id:'clean',   name:'PURGE_SYSTEM_CACHE',     icon:'🧹', desc:'Free memory — purge target/ artifacts', directive:'run cargo clean to free disk space' },
        { id:'ghost',   name:'WAKE_GHOST_CORTEX',      icon:'👻', desc:'Initiate deep neural reasoning',        directive:'summarize the last 10 memory strands and provide a system health assessment' },
        { id:'check',   name:'CARGO_CHECK',            icon:'🦀', desc:'Run cargo check, show all errors',      directive:'run cargo check and report all compiler errors' },
        { id:'clippy',  name:'RUN_CLIPPY',             icon:'📎', desc:'Lint check all Rust source',            directive:'run cargo clippy and list all warnings' },
        { id:'ui',      name:'BUILD_UI',               icon:'⚡', desc:'Vite build the Svelte frontend',        directive:'run npm run build in the ui directory' },
        { id:'vault',   name:'LIST_LAZARUS_VAULT',     icon:'🔐', desc:'Show all snapshots',                    directive:'list all lazarus vault snapshots and their hashes' },
        { id:'git',     name:'GIT_STATUS',             icon:'🌿', desc:'Show git status and pending changes',   directive:'run git status and show all changed files' },
        { id:'todo',    name:'SCAN_TODOS',             icon:'📝', desc:'Find all TODO/FIXME comments',          directive:'find all TODO FIXME HACK and SUTURE comments in the source code' },
    ];

    let commands = $derived(
        query.trim()
            ? ALL_COMMANDS.filter(c =>
                c.name.toLowerCase().includes(query.toLowerCase()) ||
                c.desc.toLowerCase().includes(query.toLowerCase())
              )
            : ALL_COMMANDS
    );

    // Reset selection when filtered list changes
    $effect(() => { if (commands) selectedIndex = 0; });

    async function fireCommand(cmd) {
        SoundFx.success();
        visible = false;
        query   = '';
        TaskService.log(`>> [PALETTE] Firing: ${cmd.name}`, 'text-cyan-400');
        try {
            await fetch('/api/task', {
                method:  'POST',
                headers: { 'Content-Type': 'application/json' },
                body:    JSON.stringify({ directive: cmd.directive }),
            });
        } catch (e) {
            TaskService.log(`!! [PALETTE] ${e.message}`, 'text-rose-400');
        }
    }

    function handleKeydown(e) {
        if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
            e.preventDefault();
            visible = !visible;
            if (visible) { SoundFx.click(); query = ''; selectedIndex = 0; }
        }
        if (!visible) return;
        if (e.key === 'Escape')    { visible = false; }
        if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = (selectedIndex+1) % commands.length; }
        if (e.key === 'ArrowUp')   { e.preventDefault(); selectedIndex = (selectedIndex-1+commands.length) % commands.length; }
        if (e.key === 'Enter' && commands[selectedIndex]) fireCommand(commands[selectedIndex]);
    }

    onMount(() => {
        window.addEventListener('keydown', handleKeydown);
        return () => window.removeEventListener('keydown', handleKeydown);
    });
</script>

{#if visible}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="palette-overlay" transition:fade={{duration:100}} onclick={() => visible=false}>
        <div class="palette-box glass-panel" transition:scale={{start:0.95,duration:150}}
             onclick={(e) => e.stopPropagation()}>
            <div class="search-row">
                <span class="prompt">❯</span>
                <input bind:value={query}
                       placeholder="FILTER DIRECTIVES..."
                       autofocus
                       class="search-input" />
                <span class="hint">ESC to close</span>
            </div>
            <div class="results">
                {#if commands.length === 0}
                    <div class="no-results">No commands match "{query}"</div>
                {/if}
                {#each commands as cmd, i}
                    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
                    <div class="result-item" class:selected={i === selectedIndex}
                         role="option" aria-selected={i === selectedIndex}
                         onclick={() => fireCommand(cmd)}
                         onmouseenter={() => selectedIndex = i}>
                        <span class="icon">{cmd.icon}</span>
                        <div class="info">
                            <div class="name">{cmd.name}</div>
                            <div class="desc">{cmd.desc}</div>
                        </div>
                        {#if i === selectedIndex}
                            <span class="enter-hint">↵ FIRE</span>
                        {/if}
                    </div>
                {/each}
            </div>
            <div class="footer">
                <span>Ctrl+K to toggle</span>
                <span>↑↓ navigate</span>
                <span>↵ execute</span>
            </div>
        </div>
    </div>
{/if}

<style>
    .palette-overlay { position:fixed; inset:0; background:rgba(0,0,0,.7); backdrop-filter:blur(4px); z-index:2000; display:flex; justify-content:center; padding-top:15vh; }
    .palette-box { width:520px; height:fit-content; background:#05050a; border:1px solid rgba(0,255,255,.2); border-radius:8px; overflow:hidden; box-shadow:0 20px 50px rgba(0,0,0,1); }
    .search-row { display:flex; align-items:center; padding:15px; border-bottom:1px solid rgba(255,255,255,.05); gap:12px; }
    .prompt { color:#00ffff; font-weight:900; font-size:14px; }
    .search-input { flex:1; background:none; border:none; color:#fff; outline:none; font-family:'JetBrains Mono',monospace; font-size:14px; }
    .hint { font-size:8px; color:rgba(255,255,255,.2); white-space:nowrap; }
    .results { padding:8px; max-height:360px; overflow-y:auto; }
    .no-results { padding:20px; text-align:center; font-size:10px; color:rgba(255,255,255,.2); }
    .result-item { display:flex; align-items:center; padding:10px 15px; gap:15px; border-radius:4px; transition:.1s; cursor:pointer; }
    .result-item.selected { background:rgba(0,255,255,.1); }
    .icon { font-size:18px; width:25px; text-align:center; }
    .info { flex:1; }
    .name { font-size:10px; font-weight:900; color:#e2e8f0; }
    .desc { font-size:8px; color:#64748b; margin-top:2px; }
    .enter-hint { font-size:8px; color:#00ffff; font-weight:900; opacity:.7; }
    .footer { display:flex; justify-content:center; gap:20px; padding:8px; border-top:1px solid rgba(255,255,255,.04); font-size:8px; color:rgba(255,255,255,.15); }
</style>
