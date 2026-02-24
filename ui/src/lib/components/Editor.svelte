<script>
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from "codemirror";
    import { EditorState } from "@codemirror/state";
    import { rust } from "@codemirror/lang-rust";
    import { oneDark } from "@codemirror/theme-one-dark";
    import { vim } from '@replit/codemirror-vim';

    export let value = "";
    export let path = "src/main.rs";
    export let onChange = (val) => {};
    export let vimMode = false;

    let editorElement;
    let view;

    function mountEditor() {
        const extensions = [
            basicSetup,
            rust(),
            oneDark,
            EditorView.updateListener.of(update => {
                if (update.docChanged) onChange(update.state.doc.toString());
            }),
            EditorView.theme({
                "&": { height: "100%" },
                ".cm-scroller": { overflow: "auto", paddingBottom: "120px" }
            })
        ];
        if (vimMode) extensions.push(vim({}));

        view = new EditorView({
            state: EditorState.create({ doc: value, extensions }),
            parent: editorElement
        });
    }

    $: if (editorElement && (vimMode !== undefined)) {
        if (view) view.destroy();
        mountEditor();
    }

    $: if (view && value !== view.state.doc.toString()) {
        let cursor = view.state.selection.main.head;
        if (cursor > value.length) cursor = value.length;
        view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: value },
            selection: { anchor: cursor }
        });
    }

    onDestroy(() => { if (view) view.destroy(); });
</script>

<div class="editor-container h-full w-full bg-black flex flex-col overflow-hidden">
    <div class="w-full bg-[#0a0a0a] border-b border-blue-900/40 px-4 py-1.5 flex items-center z-[60] gap-4">
        <div class="flex items-center space-x-3 min-w-0">
            <div class="w-1.5 h-1.5 rounded-full {path === 'NO_FILE_LOADED' ? 'bg-red-500' : 'bg-blue-500 animate-pulse'} shrink-0"></div>
            <span class="text-[11px] text-[#58a6ff] font-mono truncate">{path}</span>
        </div>
    </div>
    <div class="flex-grow overflow-hidden relative" bind:this={editorElement}></div>
</div>

<style>
    .editor-container { height: 100%; }
    :global(.cm-editor) { height: 100%; outline: none !important; background-color: #000 !important; }
</style>
