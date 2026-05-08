<script>
    import { onMount, onDestroy } from 'svelte';
    import { ide } from '$lib/core/store.svelte';
    import { EditorView, basicSetup } from 'codemirror';
    import { EditorState } from '@codemirror/state';
    import { rust } from '@codemirror/lang-rust';
    import { oneDark } from '@codemirror/theme-one-dark';
    import { vim } from '@replit/codemirror-vim';
    import { Compartment } from '@codemirror/state';

    let { path = '', vimEnabled = true } = $props();
    let container = $state();
    let view;
    // Compartment allows dynamically reconfiguring extensions without recreating the editor
    let vimCompartment = new Compartment();

    onMount(() => {
        const tab = ide.openTabs.find(t => t.path === path);
        const startState = EditorState.create({
            doc: tab?.content ?? '',
            extensions: [
                basicSetup,
                rust(),
                oneDark,
                vimCompartment.of(vimEnabled ? vim() : []),
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        ide.updateContent(path, update.state.doc.toString());
                    }
                })
            ]
        });

        view = new EditorView({
            state: startState,
            parent: container
        });
    });

    onDestroy(() => { if (view) view.destroy(); });

    // React to Vim toggle
    $effect(() => {
        if (view) {
            view.dispatch({
                effects: vimCompartment.reconfigure(vimEnabled ? vim() : [])
            });
        }
    });

    // React to tab switching/content updates
    $effect(() => {
        if (view && path) {
            const tab = ide.openTabs.find(t => t.path === path);
            const currentDoc = view.state.doc.toString();
            if (tab && tab.content !== currentDoc) {
                view.dispatch({
                    changes: { from: 0, to: currentDoc.length, insert: tab.content }
                });
            }
        }
    });
</script>

<div class="editor-wrap">
    <div bind:this={container} class="cm-container"></div>
</div>

<style>
    .editor-wrap { height: 100%; width: 100%; background: #010106; overflow: hidden; }
    .cm-container { height: 100%; width: 100%; font-size: 13px; }
    :global(.cm-editor) { height: 100% !important; }
    :global(.cm-scroller) { font-family: 'JetBrains Mono', 'Fira Code', monospace !important; }
</style>
