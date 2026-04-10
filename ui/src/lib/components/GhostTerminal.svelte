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
    let { logs = [], isThinking = false } = $props();
</script>

<div class="h-full flex flex-col bg-black p-4 font-mono text-xs border-t border-[#30363d]">
    <div class="flex-1 overflow-y-auto space-y-1 custom-scrollbar">
        {#each logs as log}
            <div class="flex gap-3">
                <span class="text-zinc-600 shrink-0">[{log.timestamp}]</span>
                <span class={log.level === 'ERROR' ? 'text-red-500' : 'text-green-400/90'}>
                    {log.message}
                </span>
            </div>
        {/each}
        {#if isThinking}
            <div class="text-blue-400 animate-pulse">&gt;&gt; CONDUCTOR_ANALYZING_NEURAL_DNA...</div>
        {/if}
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 3px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1f2937; }
</style>
