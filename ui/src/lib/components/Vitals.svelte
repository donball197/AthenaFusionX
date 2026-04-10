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
    let { ram = "0GB / 0GB", status = "OFFLINE", cpu = 0, resonance = 0.0 } = $props();

    let ramPercent = $derived(
        ram.includes('/') 
            ? (parseFloat(ram.split('/')[0]) / parseFloat(ram.split('/')[1].replace('GB',''))) * 100 || 0 
            : 0
    );
</script>

<div class="font-mono text-xs bg-[#050505] border border-[#1f1f1f] p-5 space-y-6">
    <div class="flex justify-between border-b border-[#1f1f1f] pb-3">
        <span class="text-gray-500">SYSTEM_NODE</span>
        <span class={status === 'ONLINE' ? 'text-neon-green' : 'text-red-500'}>{status}</span>
    </div>

    <div class="space-y-4">
        <div>
            <div class="flex justify-between text-gray-400 mb-1.5">
                <span>CPU_LOAD</span>
                <span>{cpu.toFixed(1)}%</span>
            </div>
            <div class="h-1 bg-[#111] rounded">
                <div class="h-full bg-neon-green transition-all" style="width: {cpu}%"></div>
            </div>
        </div>

        <div>
            <div class="flex justify-between text-gray-400 mb-1.5">
                <span>RAM_INDEX</span>
                <span>{ram}</span>
            </div>
            <div class="h-1 bg-[#111] rounded">
                <div class="h-full bg-blue-500 transition-all" style="width: {ramPercent}%"></div>
            </div>
        </div>

        <div>
            <div class="flex justify-between text-gray-400 mb-1.5">
                <span>NEURAL RESONANCE</span>
                <span class={resonance > 0.6 ? 'text-purple-400' : 'text-gray-400'}>{resonance.toFixed(3)}</span>
            </div>
            <div class="h-1 bg-[#111] rounded">
                <div class="h-full bg-purple-500 transition-all" style="width: {resonance * 100}%"></div>
            </div>
        </div>
    </div>
</div>

<style>
    .text-neon-green { color: #39FF14; }
    .bg-neon-green { background-color: #39FF14; }
</style>
