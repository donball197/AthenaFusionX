<script>
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from "codemirror";
    import { EditorState } from "@codemirror/state";
    import { rust } from "@codemirror/lang-rust";
    import { oneDark } from "@codemirror/theme-one-dark";
    
    export let value = "";
    export let onChange = (val) => {};
    
    let editorElement;
    let view;

    onMount(() => {
        let state = EditorState.create({
            doc: value,
            extensions: [
                basicSetup,
                rust(),
                oneDark,
                EditorView.updateListener.of((u) => {
                    if (u.docChanged) {
                        const val = u.state.doc.toString();
                        onChange(val);
                    }
                }),
                // THE SCROLL FIX: Injected as a CodeMirror Theme
                EditorView.theme({
                    "&": { height: "100%" },
                    ".cm-scroller": { 
                        overflow: "auto",
                        paddingBottom: "180px" // Creates the space above Ghost Terminal
                    },
                    ".cm-content": { 
                        paddingBottom: "180px" 
                    }
                })
            ]
        });

        view = new EditorView({
            state,
            parent: editorElement
        });

        return () => view.destroy();
    });

    // Handle external updates (file switching)
    $: if (view && value !== view.state.doc.toString()) {
        view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: value }
        });
    }
</script>

<div class="editor-container h-full w-full bg-[#282c34]" 
     bind:this={editorElement} 
     role="textbox" 
     aria-label="Rust Source Code Editor">
</div>

<style>
    .editor-container {
        display: flex;
        flex-direction: column;
    }

    /* Global Overrides for CM6 */
    :global(.cm-editor) {
        height: 100%;
        max-height: 100%;
        outline: none !important;
    }

    :global(.cm-content) {
        font-family: 'JetBrains Mono', monospace !important;
        font-size: 13px !important;
    }
</style>
