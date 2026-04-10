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
    import SoundFx from '$lib/SoundFx.js';

    let { response = "", isThinking = false, patchAvailable = false, onApplyPatch } = $props();
</script>

<div class="h-full flex flex-col">
    <div class="text-purple-400 text-xs border-b border-purple-900/40 pb-2 mb-3 flex justify-between uppercase tracking-widest">
        <span>GEMMA // NEURAL CORE</span>
        {#if isThinking}
            <span class="animate-pulse text-purple-500 text-[9px]">● LIVE</span>
        {/if}
    </div>

    <div class="flex-1 overflow-y-auto p-4 bg-[#020202] border border-blue-900/20 font-mono text-sm text-neon-green leading-relaxed whitespace-pre-wrap custom-scrollbar shadow-inner">
        {response || ">> UPLINK STABLE. AWAITING DIRECTIVE..."}
    </div>

    {#if patchAvailable}
        <button 
            onclick={() => { SoundFx.success?.(); onApplyPatch(); }}
            class="mt-3 py-2.5 border border-blue-500 text-blue-400 hover:bg-blue-950 font-bold text-xs tracking-widest transition-all">
            >> COMMIT PATCH TO CORE
        </button>
    {/if}
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 4px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e40af; }
    .text-neon-green { color: #39FF14; }
</style>
