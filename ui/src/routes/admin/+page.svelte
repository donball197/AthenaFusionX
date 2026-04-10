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
    import MemoryVault from '$lib/components/MemoryVault.svelte';
</script>

<div class="h-full bg-black p-6 flex flex-col gap-4">
    <header class="flex justify-between items-center border-b border-purple-900/30 pb-4">
        <h1 class="text-purple-400 font-black tracking-[4px] uppercase italic">System_Administrator</h1>
        <span class="text-[10px] text-purple-900">LEVEL_0_ACCESS_GRANTED</span>
    </header>
    <div class="flex-1 border border-purple-900/20 bg-purple-950/5">
        <MemoryVault />
    </div>
</div>
