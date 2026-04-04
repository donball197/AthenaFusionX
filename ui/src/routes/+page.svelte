<script>
    import { activeTab, layout } from '$lib/store';
    import Editor from '$lib/components/Editor.svelte';
    import Explorer from '$lib/components/Explorer.svelte';
</script>

<div class="h-full flex bg-[#020202]">
    {#if $layout.sidebarVisible}
        <div class="w-64 border-r border-cyan-900/20 shrink-0">
            <Explorer open={true} onFileSelect={(p) => activeTab.set(p)} />
        </div>
    {/if}

    <div class="flex-1 flex flex-col min-w-0">
        <Editor path={$activeTab} />
    </div>
</div>
