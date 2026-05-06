<script>
    import { uplink } from '$lib/services/uplink.js';
    
    let { activeFile } = $props();
    
    let fileContent = $state('');
    let loading = $state(false);
    let saving = $state(false);

    $effect(() => {
        if (activeFile) loadFile(activeFile);
    });

    async function loadFile(path) {
        loading = true;
        try {
            const res = await uplink.fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
            if (res.ok) {
                const data = await res.json();
                fileContent = data.content !== undefined ? data.content : '';
            }
        } catch (e) { console.error("EDITOR_LOAD_FAULT:", e); } 
        finally { loading = false; }
    }

    async function saveFile() {
        if (!activeFile) return;
        saving = true;
        try {
            await uplink.fetch(`/api/fs/write`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: activeFile, content: fileContent })
            });
        } catch (e) { console.error("EDITOR_SAVE_FAULT:", e); } 
        finally { saving = false; }
    }

    function handleKeydown(e) {
        if (e.key === 'Tab') {
            e.preventDefault();
            const start = e.target.selectionStart;
            const end = e.target.selectionEnd;
            fileContent = fileContent.substring(0, start) + "    " + fileContent.substring(end);
            setTimeout(() => {
                e.target.selectionStart = e.target.selectionEnd = start + 4;
            }, 0);
        }
        if ((e.ctrlKey || e.metaKey) && e.key === 's') {
            e.preventDefault();
            saveFile();
        }
    }
</script>

<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded shadow-lg font-mono text-sm">
    <div class="flex items-center justify-between px-4 py-2 bg-[#161b22] border-b border-gray-800 shadow-sm">
        <span class="text-purple-400 text-xs font-semibold tracking-wider">
            AST_EDITOR // {activeFile || 'NO_FILE_SELECTED'}
        </span>
        <button onclick={saveFile} disabled={!activeFile || saving} class="text-xs text-gray-400 hover:text-green-400 transition-colors disabled:opacity-50">
            {saving ? '[SAVING...]' : '[SAVE]'}
        </button>
    </div>
    <div class="flex-1 relative bg-[#0d1117]">
        {#if loading}
            <div class="absolute inset-0 flex items-center justify-center text-gray-500 bg-[#0d1117]/80 z-10">Decrypting DNA...</div>
        {/if}
        <textarea 
            bind:value={fileContent} 
            onkeydown={handleKeydown}
            disabled={!activeFile}
            spellcheck="false"
            class="w-full h-full p-4 bg-transparent text-gray-300 resize-none outline-none focus:ring-0 leading-relaxed font-mono whitespace-pre"
        ></textarea>
    </div>
</div>
