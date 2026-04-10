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
    let { files = [], onFileSelect } = $props();
    let focusedIndex = $state(0);

    function handleKeydown(e) {
        if (e.key === 'ArrowDown') {
            e.preventDefault();
            focusedIndex = Math.min(focusedIndex + 1, files.length - 1);
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            focusedIndex = Math.max(focusedIndex - 1, 0);
        } else if (e.key === 'Enter') {
            e.preventDefault();
            if (files[focusedIndex]) onFileSelect(files[focusedIndex].path);
        }
    }
</script>

<div 
    role="tree" 
    tabindex="0" 
    aria-label="Project Topology"
    onkeydown={handleKeydown}
    class="font-mono text-[10px] space-y-1 focus:outline-none focus:ring-1 focus:ring-cyan-900/50 p-2"
>
    {#each files as file, i}
        <div 
            role="treeitem" 
            aria-selected={i === focusedIndex}
            class="flex items-center gap-2 cursor-pointer transition-colors {i === focusedIndex ? 'text-cyan-400 bg-cyan-950/20' : 'text-gray-500 hover:text-gray-300'}"
            onclick={() => { focusedIndex = i; onFileSelect(file.path); }}
        >
            <span class="text-cyan-900">{file.is_dir ? '📁' : '📄'}</span>
            <span class="truncate">{file.name}</span>
        </div>
    {/each}
</div>
