<script>
    export let open = false;
    export let onFileSelect;
    
    let files = [];
    let currentDir = ".";

    async function fetchFiles(path = ".") {
        try {
            const res = await fetch(`/api/fs/list?path=${encodeURIComponent(path)}`);
            if (res.ok) {
                files = await res.json();
                currentDir = path;
            }
        } catch (e) { console.error(e); }
    }

    $: if (open && files.length === 0) fetchFiles();

    function handleBack() {
        fetchFiles(".");
    }
</script>

<aside class="absolute left-0 top-0 bottom-0 bg-[#0d1117] border-r border-green-900/30 z-[100] transition-all duration-300 {open ? 'w-64 shadow-[10px_0_30px_rgba(0,0,0,0.9)]' : 'w-0'}" style="overflow-x: hidden;">
    <div class="w-64 p-4 flex flex-col h-full">
        <div class="text-[10px] text-green-700 mb-4 border-b border-green-900/20 pb-1 flex justify-between items-center">
            <span class="tracking-widest uppercase">Filesystem</span>
            <button on:click={handleBack} class="text-[8px] text-green-500 hover:text-white">ROOT</button>
        </div>
        
        <div class="flex-1 overflow-y-auto flex flex-col gap-1 custom-scrollbar">
            {#each files as file}
                <button on:click={() => file.is_dir ? fetchFiles(file.path) : onFileSelect(file.path)} 
                        class="text-left text-[11px] p-2 rounded hover:bg-green-900/10 truncate group flex items-center gap-2">
                    <span class="opacity-50 group-hover:opacity-100">{file.is_dir ? '📁' : '📄'}</span>
                    <span class={file.is_dir ? 'text-green-600' : 'text-gray-400'}>{file.name}</span>
                </button>
            {/each}
        </div>
    </div>
</aside>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 2px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #064e3b; }
</style>
