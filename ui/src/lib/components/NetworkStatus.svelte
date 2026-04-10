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
    let { latency = 23, nodes = 47, status = "STABLE" } = $props();
</script>

<div class="bg-[#050505] border border-[#30363d] p-5 text-xs font-mono">
    <div class="uppercase tracking-widest text-cyan-400 text-[10px] mb-4">NETWORK_STATUS</div>
    
    <div class="space-y-5">
        <div class="flex justify-between">
            <span class="text-gray-400">LATENCY</span>
            <span class="text-cyan-400">{latency}ms</span>
        </div>
        <div class="flex justify-between">
            <span class="text-gray-400">ACTIVE_NODES</span>
            <span class="text-cyan-400">{nodes}</span>
        </div>
        <div class="flex justify-between">
            <span class="text-gray-400">LINK_STATE</span>
            <span class={status === 'STABLE' ? 'text-neon-green' : 'text-red-400'}>{status}</span>
        </div>
    </div>
    
    <div class="h-1.5 bg-[#1a1a1a] mt-6 rounded-full overflow-hidden">
        <div class="h-full w-[87%] bg-cyan-400"></div>
    </div>
</div>
