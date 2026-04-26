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
    import { TaskService } from '$lib/services/TaskService';

    async function runDirective(directive) {
        TaskService.log(`>> [WORKSPACE] Queuing: ${directive}`, 'text-cyan-400');
        try {
            const res = await fetch('/api/task', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ directive })
            });

            if (res.ok) {
                TaskService.log('>> [WORKSPACE] Task queued — watch terminal for output.', 'text-emerald-400');
            } else {
                TaskService.log(`!! [WORKSPACE] Server error: ${res.status}`, 'text-rose-400');
            }
        } catch (e) {
            TaskService.log(`!! [WORKSPACE] Fault: ${e.message}`, 'text-rose-400');
        }
    }
</script>

<div class="p-8 h-full bg-black text-cyan-500 font-mono overflow-auto">
    <h1 class="text-lg font-bold mb-6 border-b border-cyan-900/40 pb-2 italic uppercase tracking-widest">
        WORKSPACE_DNA_MAP
    </h1>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
        <!-- BUILD PIPELINE -->
        <div class="p-5 border border-cyan-900/30 bg-cyan-950/10 rounded">
            <h2 class="text-[10px] text-cyan-400 font-black tracking-widest mb-4">BUILD_PIPELINE</h2>
            <div class="space-y-2">
                <button onclick={() => runDirective('run cargo check and report all errors')}
                        class="ws-btn">▶ CARGO CHECK</button>
                <button onclick={() => runDirective('run npm run build in the ui directory')}
                        class="ws-btn">▶ UI BUILD</button>
                <button onclick={() => runDirective('run cargo build --release and report binary size')}
                        class="ws-btn">▶ RELEASE BUILD</button>
            </div>
        </div>

        <!-- NEURAL OPS -->
        <div class="p-5 border border-purple-900/30 bg-purple-950/10 rounded">
            <h2 class="text-[10px] text-purple-400 font-black tracking-widest mb-4">NEURAL_OPS</h2>
            <div class="space-y-2">
                <button onclick={() => runDirective('audit all rust source files for security vulnerabilities')}
                        class="ws-btn">▶ SECURITY AUDIT</button>
                <button onclick={() => runDirective('find all TODO FIXME HACK and SUTURE comments in source')}
                        class="ws-btn">▶ SCAN TODOS</button>
                <button onclick={() => runDirective('run cargo clippy and list all warnings')}
                        class="ws-btn">▶ CLIPPY SCAN</button>
            </div>
        </div>

        <!-- LAZARUS OPS -->
        <div class="p-5 border border-amber-900/30 bg-amber-950/10 rounded">
            <h2 class="text-[10px] text-amber-400 font-black tracking-widest mb-4">LAZARUS_OPS</h2>
            <div class="space-y-2">
                <button onclick={() => runDirective('take a lazarus snapshot of src/main.rs tagged manual')}
                        class="ws-btn">▶ SNAPSHOT MAIN</button>
                <button onclick={() => runDirective('list all lazarus vault snapshots and their hashes')}
                        class="ws-btn">▶ LIST VAULT</button>
            </div>
        </div>
    </div>
</div>

<style>
    .ws-btn {
        width: 100%;
        text-align: left;
        padding: 10px 12px;
        background: transparent;
        border: 1px solid rgba(0,255,255,.15);
        color: #94a3b8;
        font-size: 9px;
        font-family: 'JetBrains Mono', monospace;
        cursor: pointer;
        transition: all .15s;
    }
    .ws-btn:hover {
        color: #00ffff;
        border-color: rgba(0,255,255,.5);
        background: rgba(0,255,255,.05);
    }
</style>
