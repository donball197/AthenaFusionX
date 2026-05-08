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


<div class="p-8 text-xs font-mono text-gray-500 max-w-lg">
    <h1 class="text-white text-sm mb-8 font-bold underline uppercase tracking-widest">OS_Configuration</h1>
    <div class="space-y-6">
        <div class="flex justify-between border-b border-white/5 pb-2">
            <span>PRIMARY_CORTEX</span>
            <span class="text-cyan-400 font-bold">GEMMA_4_31B (API)</span>
        </div>
        <div class="flex justify-between border-b border-white/5 pb-2">
            <span>LOCAL_FAILSAFE</span>
            <span class="text-orange-500 font-bold">GEMMA_3_NANO (LLAMA_CPP)</span>
        </div>
        <div class="flex justify-between border-b border-white/5 pb-2">
            <span>FFI_TARGET</span>
            <span class="text-green-500 font-bold">ARM64_NATIVE</span>
        </div>
        <div class="flex justify-between border-b border-white/5 pb-2">
            <span>MEMORY_POLICY</span>
            <span class="text-cyan-700">MIMALLOC_LEAN</span>
        </div>
    </div>
</div>
