<script>
    import { onMount } from 'svelte';
    import SoundFx from '$lib/SoundFx.js';

    let { isOpen = $bindable(false) } = $props();
    let search = $state('');
    let selectedIndex = $state(0);

    const commands = [
        { id: 'purge', label: 'Purge All Neural Memories', icon: '🗑️', action: 'purge' },
        { id: 'strike', label: 'Initiate Hive Strike', icon: '⚡', action: 'strike' },
        { id: 'resync', label: 'Force Swarm Resync', icon: '⟳', action: 'resync' },
        { id: 'archive', label: 'Archive Current Session', icon: '💾', action: 'archive' },
        { id: 'lazarus', label: 'Activate Lazarus Timeline', icon: '⌛', action: 'lazarus' },
        { id: 'vision', label: 'Toggle Visual Cortex Feed', icon: '👁️', action: 'vision' },
        { id: 'settings', label: 'Open System Settings', icon: '⚙️', action: 'settings' }
    ];

    let filtered = $derived(
        search 
            ? commands.filter(cmd => cmd.label.toLowerCase().includes(search.toLowerCase()))
            : commands
    );

    function executeCommand(cmd) {
        SoundFx.click();
        isOpen = false;
        search = '';
        selectedIndex = 0;

        switch (cmd.action) {
            case 'purge': window.dispatchEvent(new CustomEvent('nexus:purge')); break;
            case 'strike': alert("HIVE STRIKE INITIATED"); break;
            case 'resync': alert("Swarm resynchronization complete."); break;
            case 'lazarus': window.dispatchEvent(new CustomEvent('nexus:lazarus')); break;
            case 'vision': window.dispatchEvent(new CustomEvent('nexus:toggle-vision')); break;
            case 'settings': window.dispatchEvent(new CustomEvent('nexus:settings')); break;
        }
    }

    function handleKeydown(e) {
        if (!isOpen) return;
        if (e.key === 'Escape') isOpen = false;
        else if (e.key === 'Enter' && filtered.length > 0) executeCommand(filtered[selectedIndex]);
        else if (e.key === 'ArrowDown') {
            e.preventDefault();
            selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
        }
        else if (e.key === 'ArrowUp') {
            e.preventDefault();
            selectedIndex = Math.max(selectedIndex - 1, 0);
        }
    }

    onMount(() => {
        const handleGlobal = (e) => {
            if (e.ctrlKey && e.key.toLowerCase() === 'k') {
                e.preventDefault();
                isOpen = !isOpen;
                if (isOpen) search = '';
            }
        };
        window.addEventListener('keydown', handleGlobal);
        return () => window.removeEventListener('keydown', handleGlobal);
    });

    function focusInput(node) {
        if (isOpen) node.focus();
        return {
            update() { if (isOpen) node.focus(); }
        };
    }
</script>

{#if isOpen}
    <div 
        class="fixed inset-0 bg-black/90 z-[300] flex items-start justify-center pt-32" 
        role="presentation"
        onclick={() => isOpen = false}
        onkeydown={(e) => e.key === 'Escape' && (isOpen = false)}
    >
        <div 
            class="w-full max-w-xl bg-[#0a0a0a] border border-neon-green/70 rounded shadow-2xl overflow-hidden nexus-crt"
            role="dialog"
            aria-modal="true"
            aria-label="Command Palette"
            tabindex="-1"
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
        >
            <div class="p-4 border-b border-[#30363d]">
                <input 
                    bind:value={search}
                    placeholder="Type command or action..."
                    class="w-full bg-transparent outline-none text-lg placeholder-gray-500"
                    use:focusInput
                    onkeydown={handleKeydown}
                />
            </div>

            <div class="max-h-96 overflow-y-auto custom-scrollbar p-2">
                {#each filtered as cmd, i}
                    <button
                        onclick={() => executeCommand(cmd)}
                        class="w-full text-left px-4 py-3 flex items-center gap-4 hover:bg-green-900/20 rounded transition-all {i === selectedIndex ? 'bg-green-900/40 border-l-2 border-neon-green' : ''}"
                    >
                        <span class="text-xl opacity-80">{cmd.icon}</span>
                        <span class="flex-1 text-sm">{cmd.label}</span>
                    </button>
                {/each}
                {#if filtered.length === 0}
                    <div class="p-8 text-center text-gray-500">No matching commands found</div>
                {/if}
            </div>

            <div class="p-3 border-t border-[#30363d] text-[10px] text-gray-500 flex justify-between">
                <span>↑ ↓ Navigate • Enter Execute • Esc Close</span>
                <span>NEXUS COMMAND PALETTE v1.0</span>
            </div>
        </div>
    </div>
{/if}
