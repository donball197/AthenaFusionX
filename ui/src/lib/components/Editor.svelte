<script>
    import { onMount } from 'svelte';
    import { EditorView, basicSetup } from "codemirror";
    import { EditorState } from "@codemirror/state";
    import { rust } from "@codemirror/lang-rust";
    import { oneDark } from "@codemirror/theme-one-dark";
    
    export let value = "";
    export let onChange = () => {};
    
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
                })
            ]
        });

        view = new EditorView({
            state,
            parent: editorElement
        });

        return () => view.destroy();
    });

    $: if (view && value !== view.state.doc.toString()) {
        view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: value }
        });
    }
</script>

<div class="h-full w-full overflow-hidden text-sm" bind:this={editorElement}></div>
