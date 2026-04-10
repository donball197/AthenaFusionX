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
    let { title = "Agent", status = "IDLE", onclick } = $props();

    function handleKeydown(event) {
        if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            if (onclick) onclick(event);
        }
    }
</script>

<div 
    role="button" 
    tabindex="0"
    aria-label="{title} status is {status}"
    onkeydown={handleKeydown}
    {onclick}
    class="p-4 border border-purple-900/30 bg-[#0a0510] hover:bg-purple-950/20 cursor-pointer transition-colors focus:outline-none focus:ring-1 focus:ring-purple-500"
>
    <div class="flex justify-between items-center mb-2">
        <h3 class="text-purple-400 font-bold uppercase tracking-widest text-[10px]">{title}</h3>
        <span class="text-[9px] px-2 py-0.5 rounded-sm {status === 'ACTIVE' ? 'bg-emerald-900/50 text-emerald-400' : 'bg-gray-900 text-gray-500'}">
            {status}
        </span>
    </div>
    <div class="text-gray-500 text-[10px] italic">
        <slot>Awaiting directive...</slot>
    </div>
</div>
