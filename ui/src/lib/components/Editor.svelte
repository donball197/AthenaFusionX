<script>
    import { onMount, onDestroy, untrack } from 'svelte';
    import { openTabs, activeTab, activeTabObj } from '$lib/store';
    import { EditorView, basicSetup } from 'codemirror';
    import { rust } from '@codemirror/lang-rust';
    import { oneDark } from '@codemirror/theme-one-dark';

    let { path = '' } = $props(); 
    let editorElement = $state();
    let view;

    // React to path changes using Svelte 5 Effects
    $effect(() => {
        if (path && view) {
            // SUTURE: Accessing the derived store correctly with $ prefix
            const content = $activeTabObj?.content || '';
            untrack(() => {
                if (view.state.doc.toString() !== content) {
                    view.dispatch({
                        changes: { from: 0, to: view.state.doc.length, insert: content }
                    });
                }
            });
        }
    });

    onMount(() => {
        view = new EditorView({
            doc: $activeTabObj?.content || '',
            extensions: [
                basicSetup,
                rust(),
                oneDark,
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        openTabs.update(tabs => tabs.map(t => 
                            t.path === path ? { ...t, content: update.state.doc.toString(), isDirty: true } : t
                        ));
                    }
                })
            ],
            parent: editorElement
        });
    });

    onDestroy(() => { if (view) view.destroy(); });
</script>

<div class="h-full w-full bg-[#0b0b0e] font-mono text-xs overflow-hidden" bind:this={editorElement}></div>
