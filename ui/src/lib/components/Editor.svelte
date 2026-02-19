<script>
    import { onMount, onDestroy } from 'svelte';
    import { EditorView, basicSetup } from "codemirror";
    import { EditorState } from "@codemirror/state";
    import { rust } from "@codemirror/lang-rust";
    import { oneDark } from "@codemirror/theme-one-dark";
    import { vim } from '@replit/codemirror-vim';

    export let value = "";
    export let path = "NO_FILE_LOADED";
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
            basicSetup, rust(), oneDark,
            EditorView.updateListener.of((u) => {
                if (u.docChanged) onChange(u.state.doc.toString());
            }),
            EditorView.theme({
                "&": { height: "100%" },
                ".cm-scroller": { overflow: "auto", paddingBottom: "180px" }
            })
        ];
        if (vimMode) extensions.push(vim());
        view = new EditorView({
            state: EditorState.create({ doc: value, extensions }),
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
    onDestroy(() => { if (view) view.destroy(); });
</script>

<div class="editor-container h-full w-full bg-black flex flex-col overflow-hidden">
    <div class="w-full bg-[#0a0a0a] border-b border-blue-900/40 px-4 py-1.5 grid grid-cols-[1fr_auto] items-center z-[60] gap-4">
        
        <div class="flex items-center space-x-3 min-w-0">
            <div class="w-1.5 h-1.5 rounded-full {path === 'NO_FILE_LOADED' ? 'bg-red-500' : 'bg-blue-500 animate-pulse'} shrink-0"></div>
            <span class="text-[10px] text-white truncate font-mono">{path}</span>
        </div>

        <div class="flex items-center space-x-4 shrink-0">
            <div class="flex flex-col items-end">
                <span class="text-[9px] text-green-400 font-bold uppercase font-mono">NEURAL_ID: T5V6Y9W4</span>
                <span class="text-[7px] text-blue-900 uppercase tracking-tighter">{value.split('\n').length} LINES</span>
            </div>
            <div class="flex gap-1">
                <div class="text-[9px] font-bold px-2 py-1 border border-blue-900/30 text-blue-400 uppercase rounded bg-blue-900/10">Shield</div>
                <div class="text-[9px] font-bold px-2 py-1 border border-green-500/50 text-green-500 uppercase rounded bg-green-950/20">Files</div>
            </div>
        </div>
    </div>
     
    <div class="flex-grow overflow-hidden relative" bind:this={editorElement} role="textbox" aria-label="Editor">
        <div class="absolute bottom-2 right-4 z-50 text-[10px] uppercase font-bold tracking-widest pointer-events-none opacity-50 {vimMode ? 'text-green-500' : 'text-blue-500'}">
            {vimMode ? '-- VIM --' : '-- INSERT --'}
        </div>
    </div>
</div>

<style>
    .editor-container { height: 100%; }
    :global(.cm-editor) { height: 100%; outline: none !important; background-color: #000 !important; }
</style>
