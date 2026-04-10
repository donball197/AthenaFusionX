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
    import { systemVitals } from '$lib/core/store.svelte.js';
</script>

<div class="flex items-center gap-6 text-[10px] font-mono text-gray-400">
    <div class="flex items-center gap-1.5">
        <span>CPU</span>
        <span class="text-cyan-400 font-medium">{Math.round((systemVitals.cpu_load || 0) * 100)}%</span>
    </div>
    <div class="flex items-center gap-1.5">
        <span>RAM</span>
        <span class="text-purple-400">{(systemVitals.ram_gb || 0).toFixed(2)}GB</span>
    </div>
    <div class="flex items-center gap-1.5">
        <span>TEMP</span>
        <span class="{systemVitals.temperature > 65 ? 'text-red-400' : 'text-orange-400'}">
            {(systemVitals.temperature || 0).toFixed(1)}°C
        </span>
    </div>
    <div class="flex items-center gap-1.5">
        <span>RES</span>
        <span class="text-purple-400">{(systemVitals.resonance || 0).toFixed(2)}</span>
    </div>
</div>
