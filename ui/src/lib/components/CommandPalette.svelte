<script>
    import { onMount } from 'svelte';
    import { pushTermLine } from '$lib/core/NexusWS.js';
    import { terminalState, systemVitals } from '$lib/core/store.svelte.js';

    let isOpen = $state(false);
    let search = $state('');
    let selectedIndex = $state(0);

    const commands = [
        { 
            id: 'status', 
            label: 'Show System Status', 
            action: () => {
                const v = systemVitals;
                pushTermLine(`>> STATUS | TEMP: ${v.temperature.toFixed(1)}°C | CPU: ${(v.cpu_load * 100).toFixed(1)}% | RAM: ${v.ram_gb.toFixed(2)} GB | RES: ${(v.resonance * 100).toFixed(1)}%`, 'text-emerald-400');
            }
        },
        { 
            id: 'clear', 
            label: 'Clear Terminal', 
            action: () => { 
                 terminalState.logs = []; 
                 pushTermLine('[NEXUS] Terminal buffer flushed.', 'text-cyan-400'); 
             }
        },
        { id: 'audit', label: 'Run Security Audit', action: () => pushTermLine('>> [AUDITOR] Initiating sovereign system scan...', 'text-cyan-400') },
        { id: 'lazarus', label: 'Activate Lazarus Vault', action: () => pushTermLine('>> [LAZARUS] Restoring timeline snapshot...', 'text-purple-400') },
        { id: 'heal', label: 'Trigger Self-Healing Cycle', action: () => pushTermLine('>> [SELF-HEALING] Autonomic repair loop engaged.', 'text-amber-400') },
        { id: 'vision', label: 'Toggle Visual Cortex', action: () => pushTermLine('>> [VISION] Visual processing layer activated.', 'text-sky-400') },
        { id: 'purge', label: 'Purge Neural Memories', action: () => pushTermLine('!! [WARNING] Memory palace purge initiated — irreversible.', 'text-rose-400') }
    ];

    let filtered = $derived(
        search ? commands.filter(c => c.label.toLowerCase().includes(search.toLowerCase())) : commands
    );

    $effect(() => {
        search;
        selectedIndex = 0;
    });

    function execute() {
        if (filtered.length === 0) return;
        filtered[selectedIndex].action();
        isOpen = false;
        search = '';
    }

    function focusInput(node) {
        setTimeout(() => node.focus(), 10);
    }

    onMount(() => {
        const handler = (e) => {
            if (e.ctrlKey && e.key.toLowerCase() === 'k') {
                e.preventDefault();
                isOpen = !isOpen;
                if (isOpen) {
                    search = '';
                    selectedIndex = 0;
                }
                return;
            }
            if (!isOpen) return;
            if (e.key === 'Escape') isOpen = false;
            if (e.key === 'Enter') { e.preventDefault(); execute(); }
            if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1); }
            if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = Math.max(selectedIndex - 1, 0); }
        };
        window.addEventListener('keydown', handler);
        return () => window.removeEventListener('keydown', handler);
    });
</script>

{#if isOpen}
    <div class="fixed inset-0 bg-black/95 z-[500] flex items-start justify-center pt-28" 
         onclick={() => isOpen = false}
         onkeydown={(e) => { if(e.key === 'Escape') isOpen = false; }}
         role="button" 
         tabindex="0">
        
        <div class="w-full max-w-2xl bg-zinc-950 border border-zinc-700 rounded-2xl shadow-2xl overflow-hidden" 
             onclick={(e) => e.stopPropagation()} 
             onkeydown={(e) => e.stopPropagation()}
             role="dialog" 
             aria-modal="true" 
             tabindex="-1">
            
            <div class="px-6 py-5 border-b border-zinc-800 flex items-center gap-3">
                <span class="text-cyan-400 text-xl">⌘</span>
                <input 
                    bind:value={search}
                    use:focusInput
                    placeholder="Search commands or directives..."
                    class="flex-1 bg-transparent text-lg text-zinc-100 outline-none font-mono placeholder-zinc-500"
                    autocomplete="off"
                    spellcheck="false"
                />
            </div>
            
            <div class="max-h-[440px] overflow-y-auto py-2 custom-scrollbar">
                {#each filtered as cmd, i}
                    <button 
                        class="w-full px-6 py-[14px] text-left font-mono text-sm transition-all flex items-center
                              {i === selectedIndex ? 'bg-zinc-900 text-white border-l-2 border-cyan-400' : 'text-zinc-400 hover:bg-zinc-900/70 hover:text-zinc-200'}"
                        onclick={() => { selectedIndex = i; execute(); }}
                        onmouseover={() => selectedIndex = i}
                        onfocus={() => selectedIndex = i}
                    >
                        <span class="flex-1">{cmd.label}</span>
                        <span class="text-[10px] uppercase tracking-widest text-zinc-600 font-light">{cmd.id}</span>
                    </button>
                {/each}
                {#if filtered.length === 0}
                    <div class="px-6 py-16 text-center text-zinc-500 font-mono text-sm">No matching directives found</div>
                {/if}
            </div>
            <div class="px-6 py-3.5 border-t border-zinc-800 bg-zinc-950/80 flex justify-between text-[10px] text-zinc-500 font-mono">
                <span>↑↓ Navigate</span>
                <span>⏎ Execute</span>
                <span>ESC Close • Ctrl+K Toggle</span>
            </div>
        </div>
    </div>
{/if}

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 4px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #3f3f46; border-radius: 2px; }
</style>
