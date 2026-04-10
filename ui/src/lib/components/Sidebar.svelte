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
    import { ide, fileTreeTrigger } from '$lib/core/store.svelte.js';

    let files = $state([]);

    async function scanDNA() {
        try {
            // We fetch from the Rust backend API
            const res = await fetch('/api/fs/list?path=.');
            if (res.ok) {
                files = await res.json();
            }
        } catch (e) {
            console.error("FS_SCAN_FAULT", e);
        }
    }

    // Refresh when the Swarm triggers it
    $effect(() => {
        if (fileTreeTrigger.value) scanDNA();
    });

    onMount(scanDNA);

    function selectSector(path) {
        if (!ide.openTabs.find(t => t.path === path)) {
            ide.openTabs.push({ path, isDirty: false });
        }
        ide.activeTab = path;
    }
</script>

<div class="sidebar-container p-2">
    <div class="text-[9px] text-cyan-900 font-black mb-4 tracking-[3px] border-b border-cyan-900/30 pb-1">DNA_STRUCTURE</div>
    <div class="flex flex-col space-y-1">
        {#each files as file}
            <button 
                onclick={() => selectSector(file.path)}
                class="group flex items-center gap-2 w-full text-left px-2 py-1.5 text-[10px] transition-all {file.is_dir ? 'text-purple-500 font-bold' : 'text-cyan-600 hover:text-cyan-300 hover:bg-cyan-900/20'}"
            >
                <span class="opacity-40 group-hover:opacity-100">{file.is_dir ? '📂' : '📄'}</span>
                <span class="truncate uppercase tracking-tighter">{file.name}</span>
            </button>
        {/each}
    </div>
</div>
