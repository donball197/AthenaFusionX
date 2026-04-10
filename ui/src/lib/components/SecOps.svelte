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
    import { pushTermLine } from '$lib/core/NexusWS.js';

    function triggerAudit() {
        pushTermLine('>> [SECOPS] Initiating deep system audit...', 'text-cyan-400');
        setTimeout(() => {
            pushTermLine('>> [SECOPS] Scanning neural pathways...', 'text-cyan-500');
        }, 800);
        setTimeout(() => {
            pushTermLine('>> [SECOPS] Audit complete. 0 anomalies detected.', 'text-green-400');
        }, 2000);
    }

    function triggerLockdown() {
        pushTermLine('!! [SECOPS] LOCKDOWN PROTOCOL INITIATED.', 'text-red-500');
    }
</script>

<div class="p-4 border border-red-900/30 bg-[#0a0202] font-mono text-xs">
    <div class="text-red-500 font-bold mb-4 uppercase tracking-widest border-b border-red-900/50 pb-2">Security Operations</div>
    
    <div class="space-y-3">
        <button 
            onclick={triggerAudit}
            class="w-full py-2 px-4 bg-cyan-950/20 border border-cyan-900/50 text-cyan-400 hover:bg-cyan-900/40 transition-colors text-left flex justify-between">
            <span>Run System Audit</span>
            <span>[A]</span>
        </button>
        
        <button 
            onclick={triggerLockdown}
            class="w-full py-2 px-4 bg-red-950/20 border border-red-900/50 text-red-500 hover:bg-red-900/40 hover:text-red-400 transition-colors text-left flex justify-between">
            <span>Initiate Lockdown</span>
            <span>[L]</span>
        </button>
    </div>
</div>
