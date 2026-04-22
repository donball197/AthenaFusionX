<script>
    import { onMount } from 'svelte';
    import { fade, scale } from 'svelte/transition';
    import SoundFx from '$lib/SoundFx.js';

    let visible = $state(false);
    let query = $state('');
    let selectedIndex = $state(0);

    const commands = [
        { id: 'audit', name: 'RUN_SECURITY_AUDIT', icon: '🛡️', desc: 'Scan workspace for vulnerabilities' },
        { id: 'build', name: 'TRIGGER_RELEASE_BUILD', icon: '📦', desc: 'Compile optimized ARM64 binary' },
        { id: 'clean', name: 'PURGE_SYSTEM_CACHE', icon: '🧹', desc: 'Free up RAM on Lenovo Duet' },
        { id: 'ghost', name: 'WAKE_GHOST_CORTEX', icon: '👻', desc: 'Initiate deep neural reasoning' }
    ];

    function handleKeydown(e) {
        if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
            e.preventDefault();
            visible = !visible;
            if (visible) SoundFx.click();
        }
        if (!visible) return;
        if (e.key === 'Escape') visible = false;
        if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = (selectedIndex + 1) % commands.length; }
        if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = (selectedIndex - 1 + commands.length) % commands.length; }
        if (e.key === 'Enter') {
            SoundFx.success();
            visible = false;
        }
    }

    onMount(() => {
        window.addEventListener('keydown', handleKeydown);
        return () => window.removeEventListener('keydown', handleKeydown);
    });
</script>

{#if visible}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div 
        class="palette-overlay" 
        transition:fade={{ duration: 100 }} 
        onclick={() => visible = false}
    >
        <div 
            class="palette-box glass-panel" 
            transition:scale={{ start: 0.95, duration: 150 }} 
            onclick={(e) => e.stopPropagation()}
        >
            <div class="search-row">
                <span class="prompt">❯</span>
                <input bind:value={query} placeholder="ENTER_SYSTEM_DIRECTIVE..." autofocus />
            </div>
            <div class="results">
                {#each commands as cmd, i}
                    <div class="result-item" class:selected={i === selectedIndex}>
                        <span class="icon">{cmd.icon}</span>
                        <div class="info">
                            <div class="name">{cmd.name}</div>
                            <div class="desc">{cmd.desc}</div>
                        </div>
                        {#if i === selectedIndex}
                            <span class="enter-hint">↵ ENTER</span>
                        {/if}
                    </div>
                {/each}
            </div>
        </div>
    </div>
{/if}

<style>
    .palette-overlay { 
        position: fixed; 
        inset: 0; 
        background: rgba(0,0,0,0.7); 
        backdrop-filter: blur(4px); 
        z-index: 2000; 
        display: flex; 
        justify-content: center; 
        padding-top: 15vh; 
    }
    .palette-box { 
        width: 500px; 
        height: fit-content; 
        background: #05050a; 
        border: 1px solid rgba(0,255,255,0.2); 
        border-radius: 8px; 
        overflow: hidden; 
        box-shadow: 0 20px 50px rgba(0,0,0,1); 
    }
    .search-row { 
        display: flex; 
        align-items: center; 
        padding: 15px; 
        border-bottom: 1px solid rgba(255,255,255,0.05); 
        gap: 15px; 
    }
    .prompt { color: #00ffff; font-weight: 900; }
    .search-row input { 
        flex: 1; 
        background: none; 
        border: none; 
        color: #fff; 
        outline: none; 
        font-family: 'JetBrains Mono', monospace; 
        font-size: 14px; 
    }
    .results { padding: 8px; }
    .result-item { 
        display: flex; 
        align-items: center; 
        padding: 10px 15px; 
        gap: 15px; 
        border-radius: 4px; 
        transition: 0.1s; 
        cursor: pointer; 
    }
    .result-item.selected { background: rgba(0, 255, 255, 0.1); }
    .icon { font-size: 18px; width: 25px; text-align: center; }
    .info { flex: 1; }
    .name { font-size: 10px; font-weight: 900; color: #e2e8f0; }
    .desc { font-size: 8px; color: #64748b; margin-top: 2px; }
    .enter-hint { font-size: 8px; color: #00ffff; font-weight: 900; opacity: 0.5; }
</style>
