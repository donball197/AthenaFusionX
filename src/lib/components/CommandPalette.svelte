<script lang="ts">
    import { onMount } from 'svelte';
    import { fade, scale } from 'svelte/transition';
    import { TaskService } from '$lib/services/TaskService';

    let visible = $state(false);
    let query = $state('');
    let selectedIndex = $state(0);

    const COMMANDS = [
        { id: 'refactor', icon: '⚡', name: 'Refactor Selection', desc: 'Optimize code structure via Ghost Cortex', dir: 'Refactor the currently selected code for ARM64 performance' },
        { id: 'explain', icon: '🧠', name: 'Explain File', desc: 'Break down current buffer logic', dir: 'Explain the architecture of the currently open file' },
        { id: 'audit', icon: '🛡️', name: 'Security Audit', desc: 'Scan for memory leaks & vulns', dir: 'Run cargo check and clippy to audit for security flaws' },
        { id: 'build', icon: '📦', name: 'Release Build', desc: 'Trigger zero-fragmentation binary compilation', dir: 'Compile optimized release binary and verify output' },
    ];

    let filtered = $derived(
        query ? COMMANDS.filter(c => c.name.toLowerCase().includes(query.toLowerCase()) || c.desc.toLowerCase().includes(query.toLowerCase())) : COMMANDS
    );

    $effect(() => { if (filtered) selectedIndex = 0; });

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
    <!-- Backdrop: Changed to a button-styled div for a11y -->
    <div class="overlay" 
         transition:fade={{duration: 100}} 
         onclick={() => visible=false}
         onkeydown={(e) => e.key === 'Escape' && (visible = false)}
         role="presentation">
        
        <div class="palette" 
             transition:scale={{start: 0.95, duration: 150}} 
             onclick={(e) => e.stopPropagation()}
             role="dialog"
             aria-modal="true"
             aria-label="Command Palette">
            
            <div class="search-bar">
                <span class="text-cyan-400" aria-hidden="true">❯</span>
                <input bind:value={query} placeholder="NEXUS COMMAND..." />
            </div>
            
            <div class="results" role="listbox">
                {#each filtered as cmd, i}
                    <button 
                        class="item" 
                        class:active={i === selectedIndex} 
                        onclick={() => execute(cmd)} 
                        onmouseenter={() => selectedIndex = i}
                        role="option" 
                        aria-selected={i === selectedIndex}>
                        <span class="icon" aria-hidden="true">{cmd.icon}</span>
                        <div class="info">
                            <div class="name">{cmd.name}</div>
                            <div class="desc">{cmd.desc}</div>
                        </div>
                    </button>
                {/each}
            </div>
        </div>
    </div>
{/if}

<style>
    .overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.8); backdrop-filter: blur(4px); z-index: 9999; display: flex; justify-content: center; padding-top: 15vh; }
    .palette { width: 560px; background: #05050a; border: 1px solid rgba(0, 255, 255, 0.2); border-radius: 8px; box-shadow: 0 20px 50px rgba(0,0,0,0.8); font-family: 'JetBrains Mono', monospace; display: flex; flex-direction: column; max-height: 60vh; }
    .search-bar { display: flex; align-items: center; padding: 16px; border-bottom: 1px solid rgba(255,255,255,0.05); gap: 12px; }
    input { flex: 1; background: transparent; border: none; color: #fff; font-family: inherit; font-size: 14px; outline: none; }
    .results { padding: 8px; overflow-y: auto; }
    .item { width: 100%; text-align: left; display: flex; align-items: center; padding: 12px; gap: 16px; border-radius: 6px; cursor: pointer; border: 1px solid transparent; background: transparent; }
    .item.active { background: rgba(0, 255, 255, 0.1); border-color: rgba(0, 255, 255, 0.2); }
    .name { font-size: 12px; font-weight: bold; color: #fff; }
    .desc { font-size: 10px; color: #64748b; margin-top: 4px; }
</style>
