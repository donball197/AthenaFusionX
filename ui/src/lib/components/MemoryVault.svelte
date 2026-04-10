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
    import { onMount, onDestroy } from 'svelte';

    let memories = $state([]);

    async function refresh() {
        try {
            const res = await fetch('/api/memories');
            if (res.ok) memories = await res.json();
        } catch (e) {
            console.error('[MemoryVault] fetch error:', e);
        }
    }

    onMount(() => {
        refresh();
        window.addEventListener('nexus:task-complete', refresh);
    });

    onDestroy(() => {
        window.removeEventListener('nexus:task-complete', refresh);
    });
</script>

<div class="h-full flex flex-col bg-[#050505] border border-purple-900/30 font-mono text-[10px]">
    <div class="p-2 border-b border-purple-900/30 flex justify-between items-center bg-[#0a0510]">
        <span class="text-purple-400 font-bold uppercase tracking-widest">Memory Palace</span>
        <button onclick={refresh} class="text-purple-600 hover:text-purple-400">⟳</button>
    </div>

    <div class="flex-1 overflow-y-auto p-2 space-y-2 custom-scrollbar">
        {#each memories as mem}
            <div class="p-2 border border-purple-900/20 bg-purple-950/5 rounded group">
                <div class="flex justify-between text-purple-500/50 mb-1">
                    <span>{mem.category ?? mem.origin ?? ''}</span>
                    <span class="text-[9px]">{mem.timestamp ?? ''}</span>
                </div>
                <div class="text-gray-300 group-hover:text-purple-300 transition-colors truncate">
                    {mem.tag ?? mem.directive ?? ''}
                </div>
                <div class="text-gray-600 text-[9px] truncate mt-1 italic">
                    {(mem.content ?? mem.result ?? '').slice(0, 120)}
                </div>
            </div>
        {/each}
        {#if memories.length === 0}
            <div class="text-center py-8 text-gray-700 italic">No persistent memories established.</div>
        {/if}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar       { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #2d1b69; }
</style>
