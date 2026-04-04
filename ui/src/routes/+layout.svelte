<script>
    import "../app.css";
    import { page } from '$app/state';
    import { systemVitals, ghostBuffer, isGhostTyping, layout } from '$lib/store';
    import { connectWS } from '$lib/components/NexusWS';
    import VitalsModule from '$lib/components/VitalsModule.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import GhostCortex from '$lib/components/GhostCortex.svelte';
    import { onMount } from 'svelte';

    let { children } = $props();
    
    // Rune for Vertical Terminal Retraction
    let terminalOpen = $state(true);

    onMount(() => connectWS());

    const navItems = [
        { name: 'IDE', path: '/' },
        { name: 'SECOPS', path: '/secops' },
        { name: 'WORKSPACE', path: '/workspace' },
        { name: 'VITALS', path: '/vitals' },
        { name: 'AUDIT', path: '/audit' },
        { name: 'AGENT-LAB', path: '/agent-lab' },
        { name: 'SETTINGS', path: '/settings' },
        { name: 'ADMIN', path: '/admin' },
        { name: 'AUTH', path: '/auth' },
        { name: 'HISTORY', path: '/history' }
    ];
</script>

<div class="h-screen w-screen bg-black text-cyan-400 font-mono flex flex-col overflow-hidden select-none">
    <header class="h-10 bg-[#050505] border-b border-cyan-900/30 flex items-center px-4 justify-between shrink-0 z-[200]">
        <div class="flex items-center gap-4 text-white">
            <span class="font-black bg-cyan-900 px-2 py-0.5 text-[10px] tracking-widest">NEXUS_OS</span>
            <nav class="flex gap-0.5">
                {#each navItems as item}
                    <a href={item.path} 
                       class="px-2 py-1 text-[9px] font-bold tracking-tighter transition-all
                       {page.url.pathname === item.path ? 'bg-cyan-500 text-black' : 'text-cyan-800 hover:text-cyan-500'}">
                        {item.name}
                    </a>
                {/each}
            </nav>
        </div>
        <VitalsModule vitals={$systemVitals} />
    </header>

    <div class="flex flex-1 overflow-hidden">
        
        <div class="flex-1 flex flex-col min-w-0 border-r border-cyan-900/20 relative">
            
            <main class="flex-1 overflow-hidden relative bg-[#020202]">
                {@render children()}
            </main>
            
            <div class="relative shrink-0 flex flex-col transition-all duration-300 ease-in-out {terminalOpen ? 'h-56' : 'h-0'}">
                
                <div class="absolute -top-6 right-4 z-[250] flex gap-1">
                    <button 
                        onclick={() => $layout.sidebarVisible = !$layout.sidebarVisible}
                        class="bg-[#050505] border border-b-0 border-cyan-900/50 text-cyan-600 hover:text-cyan-300 hover:bg-cyan-900/20 px-4 py-1 text-[9px] font-black tracking-widest uppercase transition-all rounded-t"
                    >
                        {$layout.sidebarVisible ? '◀ DNA_FS' : '▶ DNA_FS'}
                    </button>

                    <button 
                        onclick={() => terminalOpen = !terminalOpen}
                        class="bg-[#050505] border border-b-0 border-cyan-900/50 text-cyan-600 hover:text-cyan-300 hover:bg-cyan-900/20 px-4 py-1 text-[9px] font-black tracking-widest uppercase transition-all rounded-t"
                    >
                        {terminalOpen ? '▼ VISOR_UPLINK' : '▲ VISOR_UPLINK'}
                    </button>
                </div>
                
                {#if terminalOpen}
                <footer class="flex-1 bg-black border-t border-cyan-900/40 z-[100] overflow-hidden">
                    <Terminal />
                </footer>
                {/if}
            </div>
        </div>

        <aside class="w-80 bg-[#050505] flex flex-col shrink-0 z-[200]">
            <GhostCortex response={$ghostBuffer} active={$isGhostTyping} />
        </aside>
        
    </div>
</div>

<style>
    :global(body) { margin: 0; background: black; overflow: hidden; }
</style>
