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
    import { pendingPatch, ide, fileTreeTrigger } from '$lib/core/store.svelte.js';
    import { pushTermLine } from '$lib/core/NexusWS.js';
    import { writeFile } from '$lib/api.js';

    async function applyPatch() {
        if (!pendingPatch.active) return;

        const success = await writeFile(pendingPatch.path, pendingPatch.content);
        
        if (success) {
            ide.mountFile(pendingPatch.path, pendingPatch.content);
            pushTermLine(`[PATCH] Applied successfully → ${pendingPatch.path}`, 'text-emerald-400');
            fileTreeTrigger.value += 1; 
        } else {
            pushTermLine(`[PATCH] Failed to write ${pendingPatch.path}`, 'text-rose-400');
        }

        pendingPatch.active = false;
    }

    function rejectPatch() {
        pushTermLine(`[PATCH] Rejected by operator: ${pendingPatch.path}`, 'text-amber-400');
        pendingPatch.active = false;
    }
</script>

{#if pendingPatch.active}
    <div class="fixed bottom-8 right-8 w-96 bg-[#0a0a0a] border border-amber-500/70 shadow-2xl z-[300] rounded p-5 font-mono text-xs">
        <div class="flex justify-between mb-3 text-amber-400">
            <span class="font-bold tracking-widest">PATCH DETECTED</span>
            <span class="text-amber-600 truncate max-w-[150px]">{pendingPatch.path}</span>
        </div>
        
        <pre class="bg-black/80 p-3 text-[10px] text-zinc-400 overflow-auto max-h-32 mb-4 border border-amber-900 custom-scrollbar">
{pendingPatch.content?.slice(0, 300) || ''}{(pendingPatch.content?.length || 0) > 300 ? '\n...' : ''}
        </pre>

        <div class="flex gap-3">
            <button onclick={applyPatch} 
                    class="flex-1 py-3 bg-emerald-600 hover:bg-emerald-500 text-black font-bold tracking-widest text-[10px] transition-all">
                COMMIT TO CORE
            </button>
            <button onclick={rejectPatch} 
                    class="px-6 py-3 border border-rose-900 text-rose-400 hover:bg-rose-950 hover:text-rose-300 tracking-widest text-[10px] transition-all">
                REJECT
            </button>
        </div>
    </div>
{/if}

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #78350f; }
</style>
