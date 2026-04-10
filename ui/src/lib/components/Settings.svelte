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


<div class="p-8 max-w-2xl font-mono text-[11px] text-gray-400">
    <h2 class="text-white mb-6 uppercase tracking-widest border-b border-white/10 pb-2">Cognitive Configuration</h2>
    <div class="space-y-4">
        <div class="flex justify-between items-center">
            <span>Primary Cortex:</span>
            <span class="text-cyan-400">GEMMA_4_31B (API)</span>
        </div>
        <div class="flex justify-between items-center">
            <span>Local Fallback:</span>
            <span class="text-orange-400">GEMMA_3_NANO (LLAMA_CPP)</span>
        </div>
        <div class="flex justify-between items-center">
            <span>Toolchain Lock:</span>
            <span class="text-green-500">RUST_V1.80.0</span>
        </div>
    </div>
</div>
