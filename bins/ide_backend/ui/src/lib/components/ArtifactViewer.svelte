<script lang="ts">
    import { ide } from '$lib/stores/ide.svelte';
    import { onMount, onDestroy } from 'svelte';
    import { basicSetup } from 'codemirror';
    import { EditorState } from '@codemirror/state';
    import { EditorView, keymap } from '@codemirror/view';
    import { indentWithTab } from '@codemirror/commands';
    import { oneDark } from '@codemirror/theme-one-dark';

    let editorContainer = $state<HTMLElement | null>(null);
    let view = $state<EditorView | null>(null);
    let currentPath = $state<string | null>(null);

    let activeFile = $derived(ide.openTabs.find(t => t.path === ide.activeTab));

    $effect(() => {
        if (!editorContainer) return;

        if (ide.activeTab !== currentPath) {
            currentPath = ide.activeTab;
            
            if (ide.activeTab && activeFile) {
                if (!view) {
                    const state = EditorState.create({
                        doc: activeFile.content,
                        extensions: [
                            basicSetup,
                            oneDark,
                            keymap.of([indentWithTab]),
                            EditorView.updateListener.of(update => {
                                if (update.docChanged && view?.hasFocus) {
                                    ide.updateContent(ide.activeTab as string, update.state.doc.toString());
                                }
                            })
                        ]
                    });
                    view = new EditorView({ state, parent: editorContainer });
                } else {
                    view.dispatch({
                        changes: { from: 0, to: view.state.doc.length, insert: activeFile.content }
                    });
                }
            }
        } else if (view && activeFile && activeFile.content !== view.state.doc.toString() && !view.hasFocus) {
            view.dispatch({
                changes: { from: 0, to: view.state.doc.length, insert: activeFile.content }
            });
        }
    });

    onDestroy(() => { if (view) view.destroy(); });
</script>

<div class="h-full flex flex-col min-h-0 bg-[#010103] border-x border-cyan-900/30">
    <div class="flex-none p-2 border-b border-cyan-900/30 flex justify-between items-center bg-[#05050a]">
        <h2 class="text-cyan-600 font-black text-[9px] tracking-[0.2em] flex items-center gap-2 select-none">
            <span>ACTIVE_ARTIFACT</span>
            {#if ide.activeTab}
                <span class="text-cyan-400">::</span>
                <span class="text-white">{ide.activeTab}</span>
                {#if activeFile?.isDirty}
                    <span class="text-orange-400 ml-2 animate-pulse">● MODIFIED</span>
                {/if}
            {:else}
                <span class="text-cyan-400">::</span>
                <span class="text-gray-600 italic">AWAITING_AGENT_FOCUS</span>
            {/if}
        </h2>
    </div>

    <div class="flex-1 overflow-hidden min-h-0 relative">
        {#if ide.activeTab}
            <div bind:this={editorContainer} class="absolute inset-0 h-full w-full custom-cm"></div>
        {:else}
            <div class="h-full flex flex-col items-center justify-center opacity-20 font-mono text-cyan-500 select-none">
                <span class="text-4xl mb-4">⚡</span>
                <span class="tracking-[0.2em] text-xs">NO ARTIFACT IN FOCUS</span>
            </div>
        {/if}
    </div>
</div>

<style>
    :global(.custom-cm .cm-editor) { height: 100%; outline: none !important; background: transparent; }
    :global(.custom-cm .cm-scroller) { font-family: 'JetBrains Mono', monospace; font-size: 11px; }
    :global(.custom-cm .cm-gutters) { background: #010103; border-right: 1px solid rgba(0,255,255,0.05); color: rgba(0,255,255,0.3); }
    :global(.custom-cm .cm-activeLineGutter) { background: rgba(0,255,255,0.1); color: #0ff; }
    :global(.custom-cm .cm-activeLine) { background: rgba(0,255,255,0.02); }
</style>
