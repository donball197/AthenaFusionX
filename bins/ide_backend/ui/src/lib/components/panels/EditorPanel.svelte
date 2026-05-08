<script>
    /**
     * EditorPanel.svelte — v1.2 [UPLINK_SUTURE]
     * FIX: Standardized on 'uplink' for consistent authentication.
     */
    import { uplink } from '$lib/services/uplink.js';
    import { TaskService } from '$lib/services/TaskService';
    
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
            const data = await uplink.json(`/api/fs/read?path=${encodeURIComponent(path)}`);
            fileContent = data.content !== undefined ? data.content : '';
        } catch (e) { 
            TaskService.log(`!! [EDITOR] Read error: ${e.message}`, 'text-rose-400');
        } finally { 
            loading = false; 
        }
    }

    async function saveFile() {
        if (!activeFile || saving) return;
        saving = true;
        try {
            await uplink.post(`/api/fs/write`, { path: activeFile, content: fileContent });
            TaskService.log(`>> [EDITOR] Commited changes to ${activeFile}`, 'text-emerald-400');
        } catch (e) { 
            TaskService.log(`!! [EDITOR] Save error: ${e.message}`, 'text-rose-400');
        } finally { 
            saving = false; 
        }
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
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
            e.preventDefault();
            saveFile();
        }
    }
</script>

<div class="flex flex-col h-full bg-[#0d1117] border border-gray-800 rounded overflow-hidden">
    <div class="flex items-center justify-between px-4 py-2 bg-[#161b22] border-b border-gray-800">
        <span class="text-purple-400 text-[10px] font-bold tracking-widest uppercase">
            AST_Buffer // {activeFile || 'IDLE'}
        </span>
        <button 
            onclick={saveFile} 
            disabled={!activeFile || saving} 
            class="text-[9px] px-2 py-1 border border-gray-700 text-gray-400 hover:text-green-400 hover:border-green-900 transition-all disabled:opacity-30"
        >
            {saving ? 'COMMITTING...' : '[SAVE_STATE]'}
        </button>
    </div>
    <div class="flex-1 relative">
        {#if loading}
            <div class="absolute inset-0 flex items-center justify-center text-cyan-600 bg-black/60 z-10 font-mono text-xs">SYNCHRONIZING_DNA...</div>
        {/if}
        <textarea 
            bind:value={fileContent} 
            onkeydown={handleKeydown}
            disabled={!activeFile}
            spellcheck="false"
            class="w-full h-full p-6 bg-black text-gray-300 resize-none outline-none focus:ring-0 leading-relaxed font-mono text-sm whitespace-pre"
        ></textarea>
    </div>
</div>
