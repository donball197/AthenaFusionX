<script>
    import { fileTreeTrigger } from '$lib/store';
    
    let { open = false, onFileSelect } = $props();
    let files = $state([]);
    let currentDir = $state(".");

    async function fetchFiles(path = ".") {
        try {
            const res = await fetch(`/api/fs/list?path=${encodeURIComponent(path)}`);
            if (res.ok) {
                files = await res.json();
                currentDir = path;
            }
        } catch (e) {
            console.error("FS_ERROR:", e);
        }
    }

    $effect(() => {
        // Automatically refreshes when the UI opens or AI writes a file
        if (open || $fileTreeTrigger) {
            fetchFiles(currentDir);
        }
    });

    function handleBack() { fetchFiles("."); }
</script>

<aside class="h-full bg-[#0d1117] flex flex-col border-r border-cyan-900/10 select-none">
    <div class="p-3 flex flex-col h-full">
        <header class="flex justify-between items-center border-b border-cyan-900/30 pb-2 mb-4">
            <span class="text-cyan-600 text-[10px] font-black uppercase tracking-widest">DNA_FS</span>
            <button onclick={handleBack} class="text-[9px] text-cyan-700 hover:text-white transition-colors">ROOT</button>
        </header>

        <div class="flex-1 overflow-y-auto space-y-0.5 custom-scrollbar">
            {#each files as file}
                <button
                    onclick={() => file.is_dir ? fetchFiles(file.path) : onFileSelect(file.path)}
                    class="w-full text-left px-2 py-1.5 rounded hover:bg-cyan-900/20 flex items-center gap-2 text-[11px] transition-all group">
                    <span class="opacity-50">{file.is_dir ? '📁' : '📄'}</span>
                    <span class={file.is_dir ? 'text-cyan-500' : 'text-gray-400 group-hover:text-cyan-200'}>{file.name}</span>
                </button>
            {/each}
        </div>
    </div>
</aside>
