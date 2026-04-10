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
    // FIX: Migrated from Svelte 4 `export let` to Svelte 5 $props().
    let { node, level = 0 } = $props();
</script>

<!--
    FIX: `pl-{level * 4}` produces a dynamic class string that Tailwind's
    JIT compiler never generates (it can't statically analyse template
    expressions). Use an inline style instead.
-->
<div style="padding-left: {level * 16}px" class="text-xs">
    <div class="flex items-center gap-2 py-1 text-gray-300 hover:text-white cursor-default">
        {#if node.children}
            <span class="text-green-600">▾</span>
        {:else}
            <span class="text-gray-600">→</span>
        {/if}
        <span>{node.name}</span>
    </div>

    {#if node.children}
        {#each node.children as child}
            <!-- Recursive: indent increases by 1 per level -->
            <svelte:self node={child} level={level + 1} />
        {/each}
    {/if}
</div>
