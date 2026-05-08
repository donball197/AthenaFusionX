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
    import { onMount } from 'svelte';

    let snapshots = $state([]);
    let loading   = $state(true);
    let restoring = $state(null);
    let message   = $state('');

    async function refresh() {
        loading = true;
        message = '';
        try {
            const res = await fetch('/api/lazarus/snapshots');
            if (res.ok) snapshots = await res.json();
        } catch (e) {
            console.error('[History] fetch error:', e);
        } finally {
            loading = false;
        }
    }

    async function restore(hash) {
        restoring = hash;
        message = '';
        try {
            const res = await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive: `restore snapshot ${hash}` }),
            });
            if (res.ok) {
                message = `Restore queued for ${hash.slice(0, 8)} — check terminal for result.`;
            } else {
                message = `Server error: ${res.status}`;
            }
        } catch (e) {
            message = 'Restore failed: ' + e.message;
        } finally {
            restoring = null;
        }
    }

    onMount(refresh);
</script>

<div class="h-full bg-black p-6 flex flex-col gap-4 font-mono text-xs select-text">
    <header class="flex justify-between items-center border-b border-purple-900/30 pb-4 select-none">
        <div>
            <h1 class="text-purple-400 font-black tracking-[4px] uppercase italic">Lazarus_Vault</h1>
            <p class="text-purple-900 text-[9px] mt-1">Temporal Ledger // File Snapshots</p>
        </div>
        <button onclick={refresh} class="text-purple-700 hover:text-purple-400 text-lg transition-colors" aria-label="Refresh snapshots">⟳</button>
    </header>

    {#if message}
        <div class="px-4 py-2 border border-purple-700/50 text-purple-300 text-[10px]">{message}</div>
    {/if}

    {#if loading}
        <div class="text-purple-800 animate-pulse text-center py-12">Scanning vault...</div>
    {:else if snapshots.length === 0}
        <div class="text-gray-700 italic text-center py-12">No snapshots in the vault.</div>
    {:else}
        <div class="flex-1 overflow-y-auto space-y-2 custom-scrollbar">
            {#each [...snapshots].reverse() as snap}
                <div class="p-3 border border-purple-900/20 bg-purple-950/5 rounded flex justify-between items-start gap-4 group">
                    <div class="flex-1 min-w-0">
                        <div class="flex gap-3 mb-1">
                            <span class="text-purple-400 font-bold truncate">{snap.path}</span>
                            <span class="text-gray-600 shrink-0 font-mono">{snap.hash?.slice(0, 8)}</span>
                        </div>
                        <div class="text-gray-600 text-[9px]">{snap.timestamp}</div>
                    </div>
                    <button
                        onclick={() => restore(snap.hash)}
                        disabled={restoring === snap.hash}
                        class="shrink-0 px-3 py-1.5 border border-purple-800 text-purple-500 hover:bg-purple-950 hover:text-purple-300 transition-colors disabled:opacity-40 disabled:cursor-not-allowed text-[9px] tracking-widest uppercase"
                    >
                        {restoring === snap.hash ? 'QUEUING...' : 'RESTORE'}
                    </button>
                </div>
            {/each}
        </div>
    {/if}
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar       { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #2d1b69; }
</style>
