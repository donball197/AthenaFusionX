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
    import Response from './Response.svelte';

    let { response = "", isThinking = false, patchAvailable = false, onApplyPatch } = $props();
</script>

<div class="h-full flex flex-col bg-[#050505] border border-blue-900/30 rounded relative overflow-hidden">
    <div class="bg-[#0a0a0a] border-b border-blue-900/30 px-4 py-2 flex justify-between items-center">
        <span class="text-blue-400 text-xs font-bold uppercase tracking-widest">NEURAL LINK</span>
        {#if isThinking}
            <div class="flex items-center gap-1.5 text-blue-500">
                <div class="w-2 h-2 bg-blue-500 rounded-full animate-ping"></div>
                <span class="text-[9px]">PROCESSING</span>
            </div>
        {/if}
    </div>
    
    <div class="flex-1 p-4 overflow-hidden">
        <Response {response} {isThinking} {patchAvailable} {onApplyPatch} />
    </div>

    {#if patchAvailable}
        <div class="absolute bottom-0 inset-x-0 p-4 bg-gradient-to-t from-black via-black/90 to-transparent">
            <button 
                onclick={onApplyPatch}
                class="w-full py-3 bg-green-600 hover:bg-green-500 text-black font-bold text-xs tracking-widest shadow-[0_0_20px_rgba(74,222,128,0.5)] transition-all">
                [ APPLY PATCH ]
            </button>
        </div>
    {/if}
</div>
