<script>
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from "codemirror";
    import { EditorState } from "@codemirror/state";
    import { rust } from "@codemirror/lang-rust";
    import { oneDark } from "@codemirror/theme-one-dark";
    import { vim } from '@replit/codemirror-vim';

    export let value = "";
    export let path = "NO_FILE_LOADED"; // Received from parent
    export let onChange = (val) => {};
    export let vimMode = false;

    let editorElement;
    let view;

    $: if (editorElement && (vimMode === true || vimMode === false)) {
        if (view) view.destroy();
        mountEditor();
    }

    function mountEditor() {
        const extensions = [
            basicSetup,
            rust(),
            oneDark,
            EditorView.updateListener.of((u) => {
                if (u.docChanged) {
                    const val = u.state.doc.toString();
                    onChange(val);
                }
            }),
            EditorView.theme({
                "&": { height: "100%" },
                ".cm-scroller": { overflow: "auto", paddingBottom: "180px" },
                ".cm-content": { paddingBottom: "180px" }
            })
        ];

        if (vimMode) {
            extensions.push(vim());
        }

        let state = EditorState.create({
            doc: value,
            extensions: extensions
        });

        view = new EditorView({
            state,
            parent: editorElement
        });
    }

    $: if (view && value !== view.state.doc.toString()) {
        const cursor = view.state.selection.main.head;
        view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: value },
            selection: { anchor: cursor }
        });
    }

    onDestroy(() => {
        if (view) view.destroy();
    });
</script>

<div class="editor-container h-full w-full bg-[#000] relative group flex flex-col">
    <div class="w-full bg-[#0a0a0a] border-b border-blue-900/40 px-4 py-1.5 flex justify-between items-center z-[60]">
        <div class="flex items-center space-x-3">
            <div class="w-1.5 h-1.5 rounded-full {path === 'NO_FILE_LOADED' ? 'bg-red-500' : 'bg-blue-500 animate-pulse'}"></div>
            <span class="text-[9px] font-mono text-blue-400 tracking-[0.2em] uppercase opacity-70">
                LOCATION: <span class="text-white opacity-100">{path}</span>
            </span>
        </div>
        <div class="text-[9px] font-mono text-blue-900 uppercase">
            {value.split('\n').length} lines
        </div>
    </div>
     
    <div class="flex-grow overflow-hidden relative" bind:this={editorElement} role="textbox" aria-label="Editor">
        <div class="absolute bottom-2 right-4 z-50 text-[10px] uppercase font-bold tracking-widest pointer-events-none opacity-50 group-hover:opacity-100 transition-opacity {vimMode ? 'text-green-500' : 'text-blue-500'}">
            {vimMode ? '-- VIM --' : '-- INSERT --'}
        </div>
    </div>
</div>

<style>
    .editor-container { display: flex; flex-direction: column; }
    :global(.cm-editor) { height: 100%; max-height: 100%; outline: none !important; background-color: #000 !important; }
    :global(.cm-content) { font-family: 'JetBrains Mono', monospace !important; font-size: 13px !important; }
    :global(.cm-fat-cursor .cm-cursor) { background: #39FF14 !important; opacity: 0.5; }
</style>
