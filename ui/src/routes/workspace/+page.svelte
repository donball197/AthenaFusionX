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

    async function runDirective(directive) {
        pushTermLine(`>> [WORKSPACE] Queuing: ${directive}`, 'text-cyan-400');
        try {
            const res = await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive })
            });
            if (res.ok) {
                pushTermLine('>> [WORKSPACE] Task queued — watch terminal for output.', 'text-emerald-400');
            } else {
                pushTermLine(`!! [WORKSPACE] Server error: ${res.status}`, 'text-rose-400');
            }
        } catch (e) {
            pushTermLine(`!! [WORKSPACE] Fault: ${e.message}`, 'text-rose-400');
        }
    }
</script>

<div class="p-8 h-full bg-black text-cyan-500 font-mono">
    <h1 class="text-lg font-bold mb-6 border-b border-cyan-900/40 pb-2 italic uppercase">Workspace_DNA_Map</h1>
    <div class="grid grid-cols-3 gap-6">
        <div class="p-4 border border-cyan-900/30 bg-cyan-950/5">
            <h2 class="text-[10px] text-white mb-4 underline uppercase">Build_Pipeline</h2>
            <div class="space-y-2">
                <button onclick={() => runDirective('run cargo check and report any errors')}
                    class="w-full py-2 bg-cyan-900/20 border border-cyan-800 text-[10px] hover:text-white transition-all text-left px-2">> CARGO CHECK</button>
                <button onclick={() => runDirective('run npm build in the ui directory and report result')}
                    class="w-full py-2 bg-cyan-900/20 border border-cyan-800 text-[10px] hover:text-white transition-all text-left px-2">> NPM RUN BUILD</button>
                <button onclick={() => runDirective('run cargo build --release and report the binary size')}
                    class="w-full py-2 bg-cyan-900/20 border border-cyan-800 text-[10px] hover:text-white transition-all text-left px-2">> RELEASE BUILD</button>
            </div>
        </div>
        <div class="p-4 border border-purple-900/30 bg-purple-950/5">
            <h2 class="text-[10px] text-white mb-4 underline uppercase">Neural_Ops</h2>
            <div class="space-y-2">
                <button onclick={() => runDirective('audit all rust source files for security vulnerabilities')}
                    class="w-full py-2 bg-purple-900/20 border border-purple-800 text-[10px] hover:text-white transition-all text-left px-2">> SECURITY AUDIT</button>
                <button onclick={() => runDirective('search the codebase for all TODO and FIXME comments and list them')}
                    class="w-full py-2 bg-purple-900/20 border border-purple-800 text-[10px] hover:text-white transition-all text-left px-2">> SCAN TODOS</button>
                <button onclick={() => runDirective('run cargo clippy and list all warnings')}
                    class="w-full py-2 bg-purple-900/20 border border-purple-800 text-[10px] hover:text-white transition-all text-left px-2">> CLIPPY SCAN</button>
            </div>
        </div>
        <div class="p-4 border border-amber-900/30 bg-amber-950/5">
            <h2 class="text-[10px] text-white mb-4 underline uppercase">Lazarus_Ops</h2>
            <div class="space-y-2">
                <button onclick={() => runDirective('take a lazarus snapshot of src/main.rs tagged manual')}
                    class="w-full py-2 bg-amber-900/20 border border-amber-800 text-[10px] hover:text-white transition-all text-left px-2">> SNAPSHOT MAIN</button>
                <button onclick={() => runDirective('take a lazarus snapshot of all modified files')}
                    class="w-full py-2 bg-amber-900/20 border border-amber-800 text-[10px] hover:text-white transition-all text-left px-2">> SNAPSHOT ALL</button>
                <button onclick={() => runDirective('list all lazarus vault snapshots and their hashes')}
                    class="w-full py-2 bg-amber-900/20 border border-amber-800 text-[10px] hover:text-white transition-all text-left px-2">> LIST VAULT</button>
            </div>
        </div>
    </div>
</div>
