<script>
    import { onMount, onDestroy } from 'svelte';
    import { openTabs, activeTab, isGhostTyping } from '$lib/store';
    import { pushTermLine } from '$lib/components/NexusWS.js';

    export let path = '';
    let editorElement;
    let view;
    let saveStatus = '';

    // Dynamically loaded CodeMirror modules
    let EditorView, basicSetup, rust, javascript, python, markdown, oneDark;
    let ghostCursorTheme;

    function langExt(p) {
        if (!p || !rust) return [];
        if (p.endsWith('.rs'))              return rust();
        if (/\.(js|ts|svelte|jsx|tsx)$/.test(p)) return javascript();
        if (p.endsWith('.py'))              return python();
        if (/\.(md|toml|yaml|yml|json)$/.test(p)) return markdown();
        return [];
    }

    function initEditor(content = '') {
        if (!EditorView) return; // Guard for SSR/loading

        if (view) view.destroy();
        view = new EditorView({
            doc: content,
            extensions: [
                basicSetup,
                langExt(path),
                oneDark,
                ghostCursorTheme,
                EditorView.updateListener.of((update) => {
                    if (update.docChanged) {
                        openTabs.update(tabs => tabs.map(t =>
                            t.path === path
                                ? { ...t, content: view.state.doc.toString(), isDirty: true }
                                : t
                        ));
                    }
                }),
                EditorView.domEventHandlers({
                    keydown(e) {
                        if ((e.ctrlKey || e.metaKey) && e.key === 's') {
                            e.preventDefault();
                            saveCurrentFile();
                        }
                    }
                }),
            ],
            parent: editorElement,
        });
    }

    $: if (path && view && EditorView) {
        const stored = $openTabs.find(t => t.path === path)?.content ?? '';
        const current = view.state.doc.toString();
        if (current !== stored) {
            view.dispatch({ changes: { from: 0, to: current.length, insert: stored } });
        }
        if ($isGhostTyping) {
            view.dom.classList.add('ghost-active');
        } else {
            view.dom.classList.remove('ghost-active');
        }
    }

    async function saveCurrentFile() {
        const tab = $openTabs.find(t => t.path === path);
        if (!tab) return;
        saveStatus = 'saving';
        try {
            const res = await fetch('/api/fs/write', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: tab.path, content: tab.content }),
            });
            if (!res.ok) throw new Error(await res.text());
            openTabs.update(tabs => tabs.map(t =>
                t.path === path ? { ...t, isDirty: false } : t
            ));
            saveStatus = 'saved';
            pushTermLine(`[FS] Saved → ${path}`, 'ansi-green');
        } catch (err) {
            saveStatus = 'error';
            pushTermLine(`[FS] Save error: ${err.message}`, 'ansi-red');
        }
        setTimeout(() => saveStatus = '', 1800);
    }

    onMount(async () => {
        // Load CodeMirror safely (client-only)
        const cm = await import('codemirror');
        EditorView = cm.EditorView;
        basicSetup = cm.basicSetup;

        const langs = await Promise.all([
            import('@codemirror/lang-rust'),
            import('@codemirror/lang-javascript'),
            import('@codemirror/lang-python'),
            import('@codemirror/lang-markdown'),
            import('@codemirror/theme-one-dark')
        ]);

        rust = langs[0].rust;
        javascript = langs[1].javascript;
        python = langs[2].python;
        markdown = langs[3].markdown;
        oneDark = langs[4].oneDark;

        ghostCursorTheme = EditorView.theme({
            '&.ghost-active .cm-cursor': {
                borderLeftColor: '#a855f7',
                borderLeftWidth: '2px',
                animation: 'ghost-blink 0.4s step-end infinite',
            }
        });

        const content = $openTabs.find(t => t.path === path)?.content ?? '';
        initEditor(content);
    });

    let lastPath = path;
    $: if (path !== lastPath && editorElement && EditorView) {
        lastPath = path;
        const content = $openTabs.find(t => t.path === path)?.content ?? '';
        initEditor(content);
    }

    onDestroy(() => view?.destroy());
</script>

<div class="tabs">
    {#each $openTabs as tab}
    <button
        class="tab {tab.path === path ? 'active' : ''}"
        on:click={() => activeTab.set(tab.path)}
    >
        {#if tab.isDirty}<span class="tab-dot"></span>{/if}
        {tab.path.split('/').pop()}
    </button>
    {/each}
    <div class="tab-spacer"></div>
    {#if saveStatus}
        <span class="save-badge {saveStatus}">
            {saveStatus === 'saving' ? '● SAVING' : saveStatus === 'saved' ? '✓ SAVED' : '✕ ERROR'}
        </span>
    {/if}
    <button class="save-btn" on:click={saveCurrentFile} title="Save (Ctrl+S)">SAVE</button>
</div>

<div class="editor-host" bind:this={editorElement}></div>

{#if $isGhostTyping}
<div class="ghost-bar">
    <span class="ghost-dots">
        <span style="animation-delay:0ms"></span>
        <span style="animation-delay:160ms"></span>
        <span style="animation-delay:320ms"></span>
    </span>
    AI WRITING CODE…
</div>
{/if}

<style>
    .tabs { display: flex; align-items: center; background: #0a0c10; border-bottom: 1px solid #111820; overflow-x: auto; flex-shrink: 0; min-height: 28px; }
    .tabs::-webkit-scrollbar { height: 0; }
    .tab { display: flex; align-items: center; gap: 5px; padding: 5px 14px; font-size: 10px; font-family: inherit; color: #2a4050; background: none; border: none; border-right: 1px solid #0e1520; cursor: pointer; white-space: nowrap; transition: color 0.1s; }
    .tab:hover { color: #5a8090; }
    .tab.active { background: #060810; color: #22d3ee; border-bottom: 1px solid #22d3ee; }
    .tab-dot { width: 5px; height: 5px; border-radius: 50%; background: #f59e0b; }
    .tab-spacer { flex: 1; }
    .save-badge { font-size: 8px; letter-spacing: 1px; padding: 2px 8px; border-radius: 2px; white-space: nowrap; }
    .save-badge.saving { color: #f59e0b; }
    .save-badge.saved  { color: #22c55e; }
    .save-badge.error  { color: #f87171; }
    .save-btn { font-family: inherit; font-size: 8px; letter-spacing: 1px; padding: 2px 10px; margin: 0 6px; border: 1px solid #1a3040; background: none; color: #2a5060; cursor: pointer; transition: all 0.15s; }
    .save-btn:hover { border-color: #22d3ee; color: #22d3ee; }

    .editor-host { flex: 1; overflow: hidden; min-height: 0; }
    :global(.cm-editor)   { height: 100% !important; }
    :global(.cm-scroller) { font-family: 'JetBrains Mono','Fira Code',monospace !important; font-size: 11px; }

    .ghost-bar { display: flex; align-items: center; gap: 8px; padding: 4px 12px; background: #130a1e; border-top: 1px solid #3b1060; font-size: 9px; color: #9333ea; letter-spacing: 1px; flex-shrink: 0; }
    .ghost-dots { display: flex; gap: 3px; }
    .ghost-dots span { display: inline-block; width: 4px; height: 4px; border-radius: 50%; background: #7c3aed; animation: gdot 1s ease-in-out infinite; }
    @keyframes gdot { 0%,100% { transform: translateY(0); opacity: 0.4; } 50% { transform: translateY(-3px); opacity: 1; } }
    @keyframes ghost-blink { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }
</style>
