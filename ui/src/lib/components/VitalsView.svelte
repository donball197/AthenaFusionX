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
    import Heartbeat from './Heartbeat.svelte';
    import VitalsModule from './VitalsModule.svelte';
</script>

<div class="p-8 h-full bg-black flex flex-col overflow-y-auto">
    <header class="mb-8">
        <h1 class="text-cyan-400 text-2xl font-black tracking-widest">SYSTEM VITALS</h1>
        <p class="text-xs text-cyan-700">REAL-TIME HARDWARE + NEURAL RESONANCE</p>
    </header>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-6 flex-1">
        <div class="space-y-6">
            <VitalsModule />

            <div class="border border-cyan-900/30 bg-[#0a0a0a] p-6 rounded">
                <Heartbeat resonance={systemVitals.resonance} />
            </div>
        </div>

        <div class="border border-cyan-900/30 bg-[#0a0a0a] p-6 font-mono text-xs overflow-auto custom-scrollbar">
            <h2 class="text-cyan-400 mb-4 uppercase tracking-widest text-[10px]">RAW TELEMETRY</h2>
            <pre class="text-gray-400 whitespace-pre-wrap">
{JSON.stringify({
    cpu: systemVitals.cpu_load,
    ram: systemVitals.ram_gb,
    temp: systemVitals.temperature,
    res: systemVitals.resonance
}, null, 2)}
            </pre>
        </div>
    </div>
</div>

<style>
    .custom-scrollbar::-webkit-scrollbar { width: 4px; }
    .custom-scrollbar::-webkit-scrollbar-thumb { background: #1e2937; }
</style>
