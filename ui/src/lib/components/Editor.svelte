<script lang="ts">
  import { EditorState } from '@codemirror/state';
  import { EditorView, basicSetup } from 'codemirror';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { rust } from '@codemirror/lang-rust';
  import { javascript } from '@codemirror/lang-javascript';
  
  let { 
    content = $bindable(""), 
    language = "rust",
    readOnly = false 
  } = $props();
  
  let editorDiv: HTMLElement;
  let view: EditorView;

  // Initialize CodeMirror when the DOM mounts
  $effect(() => {
    if (editorDiv && !view) {
      const langExtension = language === 'rust' ? rust() : javascript();
      
      view = new EditorView({
        state: EditorState.create({
          doc: content,
          extensions: [
            basicSetup,
            oneDark,
            langExtension,
            EditorState.readOnly.of(readOnly),
            EditorView.updateListener.of((update) => {
              if (update.docChanged && !readOnly) {
                // Sync CM6 changes back to Svelte state
                content = update.state.doc.toString();
              }
            })
          ]
        }),
        parent: editorDiv
      });
    }

    // Cleanup on dismount
    return () => {
      if (view) {
        view.destroy();
        view = undefined;
      }
    };
  });

  // Reactive effect to handle external content changes (e.g., loading a new file)
  $effect(() => {
    if (view && content !== view.state.doc.toString()) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: content }
      });
    }
  });
</script>

<div class="editor-container" bind:this={editorDiv}></div>

<style>
  .editor-container {
    height: 100%;
    width: 100%;
    overflow: hidden;
    border-radius: 8px;
    border: 1px solid rgba(52, 211, 153, 0.15);
    background: #282c34;
  }
  
  /* Force CodeMirror to fill the container */
  :global(.cm-editor) {
    height: 100%;
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 13px;
  }
  
  :global(.cm-scroller) {
    font-family: inherit;
  }
</style>
