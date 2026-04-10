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
    let { resonance = 0.0 } = $props();

    let nodes = $state([]);

    onMount(async () => {
        try {
            const res = await fetch('/api/graph');
            if (res.ok) nodes = (await res.json()).nodes || [];
        } catch(e) {}
    });
</script>

<div class="h-52 border border-[#30363d] bg-[#050505] rounded relative overflow-hidden">
    <div class="absolute top-3 left-3 text-[10px] text-gray-500 tracking-widest">
        NEURAL GRAPH • RES: {resonance.toFixed(2)}
    </div>

    {#if nodes.length === 0}
        <div class="flex items-center justify-center h-full text-gray-600 text-xs italic">
            Scanning neural architecture...
        </div>
    {:else}
        <div class="absolute inset-0 flex items-center justify-center">
            <div class="grid grid-cols-5 gap-3 p-6">
                {#each nodes.slice(0, 15) as node}
                    <div class="w-3 h-3 rounded-full bg-blue-500/60 hover:bg-blue-400 transition-all" title={node.id}></div>
                {/each}
            </div>
        </div>
    {/if}
</div>
