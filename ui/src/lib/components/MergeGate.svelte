<script>
    import { pendingPatch, openTabs, fileTreeTrigger } from '$lib/store';
    import { pushTermLine } from '$lib/components/NexusWS.js';

    async function applyPatch() {
        const patch = $pendingPatch;
        if (!patch) return;

        // Write into editor store
        openTabs.update(tabs => {
            const exists = tabs.find(t => t.path === patch.path);
            if (exists) {
                return tabs.map(t => t.path === patch.path
                    ? { ...t, content: patch.content, isDirty: true }
                    : t);
            }
            return [...tabs, { path: patch.path, content: patch.content, isDirty: true }];
        });

        // Persist to disk
        try {
            const res = await fetch('/api/fs/write', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ path: patch.path, content: patch.content }),
            });
            if (!res.ok) throw new Error(await res.text());
            openTabs.update(tabs => tabs.map(t =>
                t.path === patch.path ? { ...t, isDirty: false } : t
            ));
            fileTreeTrigger.update(n => n + 1);
            pushTermLine(`[PATCH] Applied → ${patch.path}`, 'ansi-green');
        } catch (e) {
            pushTermLine(`[PATCH] Write failed: ${e.message}`, 'ansi-red');
        }

        pendingPatch.set(null);
    }

    function rejectPatch() {
        pushTermLine(`[PATCH] Rejected by operator.`, 'ansi-yellow');
        pendingPatch.set(null);
    }
</script>

{#if $pendingPatch}
<div class="fixed bottom-24 right-8 w-80 bg-[#050505] border border-yellow-500/50
            shadow-[0_0_20px_rgba(234,179,8,0.2)] z-[100] p-4 font-mono">
    <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
            <div class="w-2 h-2 bg-yellow-500 animate-pulse rounded-full"></div>
            <span class="text-[10px] text-yellow-500 font-bold uppercase tracking-widest">Patch Detected</span>
        </div>
        <span class="text-[8px] text-gray-500 truncate max-w-[120px]">{$pendingPatch.path}</span>
    </div>

    <!-- Diff preview — first 8 lines -->
    <pre class="text-[8px] text-gray-400 bg-black/60 border border-gray-800 p-2 mb-4 max-h-20 overflow-hidden leading-tight">{$pendingPatch.content.split('\n').slice(0,8).join('\n')}{$pendingPatch.content.split('\n').length > 8 ? '\n...' : ''}</pre>

    <div class="flex gap-2">
        <button
            on:click={applyPatch}
            class="flex-1 bg-yellow-500/10 border border-yellow-500 text-yellow-500 text-[10px] py-1.5
                   font-bold hover:bg-yellow-500 hover:text-black transition-all">
            COMMIT CHANGE
        </button>
        <button
            on:click={rejectPatch}
            class="px-3 bg-red-950/20 border border-red-900 text-red-700 text-[10px]
                   hover:bg-red-900 hover:text-white transition-colors">
            ✕
        </button>
    </div>
</div>
{/if}
