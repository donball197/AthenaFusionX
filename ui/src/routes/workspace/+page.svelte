<script>
    import { onMount } from 'svelte';
    import { TaskService } from '$lib/services/TaskService';
    import { bootAuth } from '$lib/stores/auth.svelte';
    import TerminalPanel from '$lib/components/panels/TerminalPanel.svelte';
    import FileExplorerPanel from '$lib/components/panels/FileExplorerPanel.svelte';
    import EditorPanel from '$lib/components/panels/EditorPanel.svelte';

    let activeFile = $state(null);

    onMount(async () => {
        await bootAuth();
    });

    async function runDirective(directive) {
        await TaskService.runDirective(directive, 'WORKSPACE');
    }
</script>

<div class="flex flex-col h-screen bg-black text-cyan-500 font-mono p-4 gap-4 overflow-hidden">
    <div class="shrink-0">
        <h1 class="text-lg font-bold mb-4 border-b border-cyan-900/40 pb-2 italic uppercase tracking-widest pl-2">
            WORKSPACE_DNA_MAP // COMMAND UPLINK
        </h1>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
            <div class="p-4 border border-cyan-900/30 bg-cyan-950/10 rounded">
                <h2 class="text-[10px] text-cyan-400 font-black tracking-widest mb-3 uppercase">Build_Substrate</h2>
                <div class="space-y-1">
                    <button onclick={() => runDirective('run cargo check')} class="ws-btn text-cyan-200">▶ CARGO CHECK</button>
                    <button onclick={() => runDirective('npm run build')} class="ws-btn">▶ UI RE-LINK</button>
                </div>
            </div>
            <div class="p-4 border border-purple-900/30 bg-purple-950/10 rounded">
                <h2 class="text-[10px] text-purple-400 font-black tracking-widest mb-3 uppercase">Neural_Context</h2>
                <div class="space-y-1">
                    <button onclick={() => runDirective('audit rust source for logic flaws')} class="ws-btn">▶ SECURITY AUDIT</button>
                    <button onclick={() => runDirective('scan for TODO and FIXME tags')} class="ws-btn">▶ SCAN SUBSTRATE</button>
                </div>
            </div>
            <div class="p-4 border border-amber-900/30 bg-amber-950/10 rounded">
                <h2 class="text-[10px] text-amber-400 font-black tracking-widest mb-3 uppercase">Lazarus_Vault</h2>
                <div class="space-y-1">
                    <button onclick={() => runDirective('list snapshots')} class="ws-btn">▶ VIEW_LEDGER</button>
                    <button onclick={() => runDirective('take manual snapshot')} class="ws-btn">▶ SAVE_STATE</button>
                </div>
            </div>
        </div>
    </div>

    <div class="flex-1 flex gap-4 min-h-0">
        <div class="w-1/5 flex flex-col min-h-0">
            <FileExplorerPanel bind:activeFile={activeFile} />
        </div>
        <div class="w-1/2 flex flex-col min-h-0">
            <EditorPanel {activeFile} />
        </div>
        <div class="w-[30%] flex flex-col min-h-0">
            <TerminalPanel />
        </div>
    </div>
</div>

<style>
    .ws-btn { 
        width:100%; text-align:left; padding:8px 10px; background:transparent; 
        border:1px solid rgba(0,255,255,.15); color:#94a3b8; font-size:10px; 
        font-family:'JetBrains Mono',monospace; cursor:pointer; transition:all .15s; 
    }
    .ws-btn:hover { color:#00ffff; border-color:rgba(0,255,255,.5); background:rgba(0,255,255,.05); }
</style>
