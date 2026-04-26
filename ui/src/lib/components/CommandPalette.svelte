<script lang="ts">
    import { onMount } from 'svelte';
    import { fade, scale } from 'svelte/transition';
    import { TaskService } from '$lib/services/TaskService';

    let visible = $state(false);
    let query = $state('');
    let selectedIndex = $state(0);
    let inputEl = $state<HTMLInputElement | null>(null);

    const COMMANDS = [
        { id: 'refactor', icon: '⚡', name: 'Refactor Selection', desc: 'Optimize code structure via Ghost Cortex', dir: 'Refactor selection for ARM64' },
        { id: 'explain', icon: '🧠', name: 'Explain File', desc: 'Break down current buffer logic', dir: 'Explain architecture' },
        { id: 'audit', icon: '🛡️', name: 'Security Audit', desc: 'Scan for memory leaks', dir: 'Run audit' },
        { id: 'build', icon: '📦', name: 'Release Build', desc: 'Trigger binary compilation', dir: 'Compile release' },
    ];

    let filtered = $derived(
        query ? COMMANDS.filter(c => c.name.toLowerCase().includes(query.toLowerCase())) : COMMANDS
    );

    $effect(() => { if (visible && inputEl) inputEl.focus(); });

    function execute(cmd: typeof COMMANDS[0]) {
        TaskService.runDirective(cmd.dir, 'PALETTE');
        visible = false;
        query = '';
    }

    function onKey(e: KeyboardEvent) {
        if ((e.ctrlKey || e.metaKey) && e.key === 'k') { e.preventDefault(); visible = !visible; }
        if (!visible) return;
        if (e.key === 'Escape') visible = false;
        if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = (selectedIndex + 1) % filtered.length; }
        if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = (selectedIndex - 1 + filtered.length) % filtered.length; }
        if (e.key === 'Enter' && filtered[selectedIndex]) execute(filtered[selectedIndex]);
    }

    onMount(() => {
        window.addEventListener('keydown', onKey);
        return () => window.removeEventListener('keydown', onKey);
    });
</script>

{#if visible}
    <div class="overlay" 
         transition:fade={{duration: 100}} 
         onclick={() => visible=false} 
         onkeydown={(e) => e.key === 'Escape' && (visible = false)}
         role="presentation">
        <div class="palette" 
             transition:scale={{start: 0.95, duration: 150}} 
             onclick={(e) => e.stopPropagation()} 
             onkeydown={(e) => { if(e.key === 'Escape') visible = false; }}
             role="dialog" 
             aria-modal="true"
             tabindex="-1">
            <div class="search-bar">
                <span class="text-cyan-400">❯</span>
                <input bind:this={inputEl} bind:value={query} placeholder="NEXUS COMMAND..." />
            </div>
            <div class="results" role="listbox">
                {#each filtered as cmd, i}
                    <button class="item" 
                            class:active={i === selectedIndex} 
                            onclick={() => execute(cmd)} 
                            onmouseenter={() => selectedIndex = i} 
                            role="option" 
                            aria-selected={i === selectedIndex}>
                        <span class="icon">{cmd.icon}</span>
                        <div class="info"><div class="name">{cmd.name}</div><div class="desc">{cmd.desc}</div></div>
                    </button>
                {/each}
            </div>
        </div>
    </div>
{/if}

<style>
    .overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.8); backdrop-filter: blur(4px); z-index: 9999; display: flex; justify-content: center; padding-top: 15vh; }
    .palette { width: 560px; background: #05050a; border: 1px solid rgba(0, 255, 255, 0.2); border-radius: 8px; font-family: 'JetBrains Mono', monospace; display: flex; flex-direction: column; max-height: 60vh; outline: none; }
    .search-bar { display: flex; align-items: center; padding: 16px; border-bottom: 1px solid rgba(255,255,255,0.05); gap: 12px; }
    input { flex: 1; background: transparent; border: none; color: #fff; outline: none; }
    .results { padding: 8px; overflow-y: auto; }
    .item { width: 100%; text-align: left; display: flex; align-items: center; padding: 12px; gap: 16px; background: transparent; color: white; border: none; cursor: pointer; }
    .item.active { background: rgba(0, 255, 255, 0.1); }
    .name { font-size: 12px; font-weight: bold; }
    .desc { font-size: 10px; color: #64748b; }
</style>
