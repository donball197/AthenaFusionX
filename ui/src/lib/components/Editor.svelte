/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


<script>
    import { onMount, tick } from 'svelte';
    import { ide } from '$lib/core/store.svelte.js';
    import { EditorState } from '@codemirror/state';
    import { EditorView, basicSetup } from 'codemirror';
    import { oneDark } from '@codemirror/theme-one-dark';
    import { rust } from '@codemirror/lang-rust';

    let { path } = $props();

    let editorContainer;
    let view;

    onMount(async () => {
        await tick();
        if (editorContainer) {
            view = new EditorView({
                state: EditorState.create({
                    doc: "// NEXUS_OS READY\n// CodeMirror initialized successfully.",
                    extensions: [
                        basicSetup,
                        oneDark,
                        rust(),
                        EditorView.updateListener.of((update) => {
                            if (update.docChanged && path) {
                                const tab = ide.openTabs.find(t => t.path === path);
                                if (tab && tab.content !== update.state.doc.toString()) {
                                    tab.content = update.state.doc.toString();
                                    tab.isDirty = true;
                                }
                            }
                        })
                    ]
                }),
                parent: editorContainer
            });
        }
        return () => view?.destroy();
    });

    $effect(() => {
        if (!view || !path) return;
        
        const currentFile = ide.openTabs.find(t => t.path === path);
        
        // SUTURE: Telemetry logged to the Chrome console
        console.log("[VISUAL_CORTEX] Target:", path, "| Content Loaded:", !!currentFile?.content);
        
        if (currentFile && currentFile.content !== undefined) {
            const newContent = currentFile.content;
            if (newContent !== view.state.doc.toString()) {
                view.dispatch({
                    changes: {
                        from: 0,
                        to: view.state.doc.length,
                        insert: newContent
                    }
                });
            }
        }
    });
</script>

<div class="editor-wrapper">
    <div class="editor-layer" bind:this={editorContainer}></div>
    
    {#if !path}
        <div class="welcome-overlay">
            AWAITING SECTOR SELECTION...
        </div>
    {/if}
</div>

<style>
    .editor-wrapper {
        position: relative;
        height: 100%;
        width: 100%;
        background: #020202;
    }
    .editor-layer {
        position: absolute;
        inset: 0;
        height: 100%;
        width: 100%;
        overflow: hidden;
    }
    .welcome-overlay {
        position: absolute;
        inset: 0;
        z-index: 10;
        display: flex;
        align-items: center;
        justify-content: center;
        background: #020202;
        color: #444;
        font-family: 'JetBrains Mono', monospace;
        letter-spacing: 2px;
        font-size: 0.9rem;
    }
    :global(.cm-editor) {
        height: 100% !important;
        outline: none !important;
    }
    :global(.cm-scroller) {
        font-family: 'JetBrains Mono', monospace !important;
        font-size: 12.5px;
    }
</style>
