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
        } catch (e) {
            console.error("Filesystem error:", e);
        }
    }

    $: if (open && files.length === 0) fetchFiles();

    function handleBack() {
        fetchFiles(".");
    }
</script>

<aside class="absolute left-0 top-0 bottom-0 bg-[#0d1117] border-r border-green-900/40 z-[110] transition-all duration-300 shadow-2xl {open ? 'w-72' : 'w-0 overflow-hidden'}">
    <div class="w-72 p-5 flex flex-col h-full">
        <div class="flex justify-between items-center border-b border-green-900/30 pb-3 mb-4">
            <span class="text-green-600 text-xs uppercase tracking-widest font-bold">FILESYSTEM</span>
            <button on:click={handleBack} class="text-[9px] text-green-500 hover:text-white">ROOT</button>
        </div>

        <div class="flex-1 overflow-y-auto custom-scrollbar space-y-0.5">
            {#each files as file}
                <button
                    on:click={() => file.is_dir ? fetchFiles(file.path) : onFileSelect(file.path)}
                    class="w-full text-left px-3 py-2.5 rounded hover:bg-green-900/20 flex items-center gap-3 text-sm transition-colors group">
                    <span class="text-lg opacity-70 group-hover:opacity-100">{file.is_dir ? '📁' : '📄'}</span>
                    <span class={file.is_dir ? 'text-green-500' : 'text-gray-300'}>{file.name}</span>
                </button>
            {/each}
        </div>
    </div>
</aside>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #0a3d2f; }
</style>
