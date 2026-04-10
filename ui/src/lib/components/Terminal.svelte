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
    import { terminalState, wsState } from '$lib/core/store.svelte.js';
    import { WS, pushTermLine } from '$lib/core/NexusWS.js';

    let viewportRef = $state(null);
    let inputRef = $state(null);
    let command = $state('');

    $effect(() => {
        // Auto-scroll when logs or the ghost buffer updates
        if ((terminalState.logs.length || terminalState.ghostBuffer) && viewportRef) {
            viewportRef.scrollTop = viewportRef.scrollHeight;
        }
    });

    function executeCommand() {
        if (!command.trim() || !wsState.ready) return;
        
        pushTermLine(`❯ ${command}`, 'text-gray-500');
        WS.send(JSON.stringify({ type: 'directive', payload: command }));
        command = '';
    }

    function handleKeydown(e) {
        if (e.key === 'Enter') executeCommand();
    }
</script>

<div class="flex flex-col h-full w-full bg-[#050508] font-mono text-[11px] p-3 overflow-hidden border border-cyan-900/20">
    <div class="flex-1 overflow-y-auto space-y-1 custom-scrollbar" bind:this={viewportRef}>
        {#each terminalState.logs as log}
            <div class="{log.cls} whitespace-pre-wrap">{log.message}</div>
        {/each}
        
        {#if terminalState.isGhostTyping}
            <div class="text-cyan-100 whitespace-pre-wrap border-l border-cyan-500 pl-2">
                {terminalState.ghostBuffer}<span class="animate-pulse">_</span>
            </div>
        {/if}
    </div>

    <div class="mt-2 pt-2 border-t border-cyan-900/20 flex items-center gap-2">
        <span class="text-cyan-600 font-bold">NEXUS_CLI ❯</span>
        <input 
            bind:this={inputRef}
            bind:value={command}
            onkeydown={handleKeydown}
            class="flex-1 bg-transparent border-none outline-none text-cyan-300 placeholder-cyan-900/50"
            placeholder="Awaiting directive..."
        />
    </div>
</div>
