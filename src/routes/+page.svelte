<script>
    let sidebarOpen = true, activeTab = 'hive', editorCode = "", currentFile = "";
    let vitals = { cpu: 0, ram: 0, status: "OFFLINE" };

    async function toggleSid() { sidebarOpen = !sidebarOpen; }

    async function openFile(path) {
        currentFile = path;
        const res = await fetch(`/api/files/read?path=${path}`);
        const data = await res.json();
        editorCode = data.content;
        sidebarOpen = false; // Retract for Zen Mode
    }
</script>

<main class="ide-root">
    <aside class:closed={!sidebarOpen}>
        <div class="sid-header">
            <span>NEXUS_SID v0.9</span>
            <button on:click={toggleSid}>◀</button>
        </div>
        </aside>

    <section class="editor-pane">
        {#if !sidebarOpen}<button on:click={toggleSid} class="expand">▶</button>{/if}
        <textarea bind:value={editorCode}></textarea>
    </section>
</main>

<style>
    .ide-root { display: flex; height: 100vh; background: #010409; color: #c9d1d9; }
    aside { width: 260px; border-right: 1px solid #30363d; transition: 0.3s; }
    aside.closed { width: 0; overflow: hidden; }
    .editor-pane { flex: 1; position: relative; }
    textarea { width: 100%; height: 100%; background: transparent; color: #e6edf3; border: none; padding: 20px; font-family: monospace; }
</style>
