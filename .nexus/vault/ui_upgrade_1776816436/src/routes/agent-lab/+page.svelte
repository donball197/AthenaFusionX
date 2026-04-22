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
    import AgentMap from '$lib/components/AgentMap.svelte';
    let swarm = $state(Array.from({length: 12}, (_, i) => ({ id: i, target: i < 3 ? "ACTIVE" : "IDLE" })));
</script>

<div class="p-8 h-full bg-black overflow-y-auto">
    <header class="flex justify-between items-baseline mb-8">
        <h1 class="text-purple-400 font-bold tracking-widest uppercase italic">Swarm_Reflex_Lab</h1>
        <span class="text-[10px] text-purple-900">NODE_COUNT: 68</span>
    </header>
    <AgentMap {swarm} />
</div>
