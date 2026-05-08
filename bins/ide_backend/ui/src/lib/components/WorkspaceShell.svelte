<script lang="ts">
    // WorkspaceShell.svelte — v1.1 [IMPORT_FIX]
    // FIX: Added missing import for terminal store — was referenced but not imported,
    //      causing a build failure in strict TypeScript mode.
    import { ui } from '$lib/stores/ui.svelte';
    import { terminal } from '$lib/stores/terminal.svelte';
    import TaskBoard from './TaskBoard.svelte';
    import CommandPalette from './CommandPalette.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
</script>

<div class="workspace-root">
    <header class="workspace-header">
        <div class="brand">NEXUS_OS <span class="badge">v16.0</span></div>
        <div class="tabs">
            <button class:active={ui.activeView === 'WORKSPACE'} onclick={() => ui.activeView = 'WORKSPACE'}>WORKSPACE</button>
            <button class:active={ui.activeView === 'TASKS'}     onclick={() => ui.activeView = 'TASKS'}>TASK PIPELINE</button>
        </div>
        <div class="hint">Ctrl+K for Commands</div>
    </header>

    <div class="workspace-body">
        {#if ui.sidebarOpen}
            <aside class="sidebar border-r border-gray-800 w-64">
                <div class="pane-header">EXPLORER</div>
                <div class="p-4 text-xs text-gray-500 italic">Project DNA Index</div>
            </aside>
        {/if}

        <main class="editor-pane flex-1">
            {#if ui.activeView === 'WORKSPACE'}
                <div class="flex items-center justify-center h-full text-gray-700 text-sm italic border border-dashed border-gray-800 m-4">
                    [CodeMirror Engine Active]
                </div>
            {:else if ui.activeView === 'TASKS'}
                <TaskBoard />
            {/if}
        </main>

        {#if ui.ghostVisible}
            <aside class="ai-console border-l border-gray-800 w-80 flex flex-col">
                <div class="pane-header flex justify-between">
                    <span>GHOST CORTEX</span>
                    <span class="text-emerald-400 text-[9px]">
                        {terminal.isGhostTyping ? 'PROCESSING…' : 'ONLINE'}
                    </span>
                </div>
                <div class="flex-1 overflow-y-auto p-4 space-y-4 font-mono text-[10px]">
                    {#each terminal.logs as log (log.timestamp + log.message)}
                        <div class="{log.cls} break-words">{log.message}</div>
                    {/each}
                    {#if terminal.isGhostTyping}
                        <div class="text-cyan-400 animate-pulse">█</div>
                    {/if}
                </div>
            </aside>
        {/if}
    </div>

    <footer class="workspace-footer border-t border-gray-800 h-48 flex flex-col">
        <div class="pane-header flex-none">INTEGRATED TERMINAL</div>
        <div class="flex-1 overflow-hidden">
            <Terminal />
        </div>
    </footer>

    <CommandPalette />
</div>

<style>
    .workspace-root   { display: flex; flex-direction: column; height: 100vh; width: 100vw; background: #000; color: #e2e8f0; font-family: 'JetBrains Mono', monospace; overflow: hidden; }
    .workspace-header { flex: none; height: 40px; display: flex; align-items: center; justify-content: space-between; padding: 0 16px; background: #05050a; border-bottom: 1px solid rgba(255,255,255,0.05); }
    .brand   { font-size: 12px; font-weight: 900; letter-spacing: 0.1em; color: #22d3ee; }
    .badge   { background: rgba(34,211,238,0.1); padding: 2px 6px; border-radius: 4px; font-size: 9px; margin-left: 8px; }
    .tabs button { background: none; border: none; color: #64748b; font-size: 10px; font-weight: bold; letter-spacing: 0.1em; padding: 0 16px; height: 100%; cursor: pointer; transition: 0.2s; font-family: 'JetBrains Mono', monospace; }
    .tabs button.active { color: #fff; border-bottom: 2px solid #22d3ee; }
    .hint { font-size: 10px; color: #475569; }
    .workspace-body  { flex: 1; display: flex; min-height: 0; }
    .pane-header     { font-size: 9px; font-weight: 900; letter-spacing: 0.1em; color: #64748b; padding: 8px 12px; background: #05050a; border-bottom: 1px solid rgba(255,255,255,0.02); }
    .sidebar, .ai-console { background: #020205; }
    .editor-pane     { background: #000; display: flex; flex-direction: column; }
    .workspace-footer { background: #020205; display: flex; flex-direction: column; }
</style>
