<script lang="ts">
    import { taskQueue } from '$lib/stores/tasks.svelte';
    import { fly } from 'svelte/transition';

    let queued = $derived(taskQueue.tasks.filter(t => t.status === 'queued'));
    let running = $derived(taskQueue.tasks.filter(t => t.status === 'running'));
    let done = $derived(taskQueue.tasks.filter(t => t.status === 'done'));
    let failed = $derived(taskQueue.tasks.filter(t => t.status === 'failed'));
</script>

<div class="task-board">
    <div class="board-header">
        <span class="title">◈ AUTONOMIC TASK PIPELINE</span>
        <span class="count">TOTAL: {taskQueue.tasks.length}</span>
    </div>

    <div class="kanban-grid">
        <div class="kanban-col">
            <div class="col-hdr text-cyan-400">⚡ RUNNING ({running.length})</div>
            <div class="col-body">
                {#each running as task (task.id)}
                    <div class="task-card border-cyan-500/50 bg-cyan-950/20" in:fly={{y: 10}}>
                        <div class="task-id">{task.id.split('-')[0]}</div>
                        <div class="task-dir">{task.directive}</div>
                        <div class="loader"></div>
                    </div>
                {/each}
            </div>
        </div>

        <div class="kanban-col">
            <div class="col-hdr text-gray-400">WAITING ({queued.length})</div>
            <div class="col-body">
                {#each queued as task (task.id)}
                    <div class="task-card border-gray-700 bg-gray-900/30" in:fly={{y: 10}}>
                        <div class="task-id">{task.id.split('-')[0]}</div>
                        <div class="task-dir">{task.directive}</div>
                    </div>
                {/each}
            </div>
        </div>

        <div class="kanban-col">
            <div class="col-hdr text-purple-400">RESOLVED ({done.length + failed.length})</div>
            <div class="col-body">
                {#each failed as task (task.id)}
                    <div class="task-card border-rose-500/50 bg-rose-950/20">
                        <div class="task-id text-rose-400">FAULT: {task.id.split('-')[0]}</div>
                        <div class="task-dir text-gray-400">{task.directive}</div>
                    </div>
                {/each}
                {#each done as task (task.id)}
                    <div class="task-card border-emerald-500/30 bg-emerald-950/10">
                        <div class="task-id text-emerald-400">OK: {task.id.split('-')[0]}</div>
                        <div class="task-dir text-gray-500">{task.directive}</div>
                    </div>
                {/each}
            </div>
        </div>
    </div>
</div>

<style>
    .task-board { display: flex; flex-direction: column; height: 100%; background: #020208; font-family: 'JetBrains Mono', monospace; }
    .board-header { padding: 12px 16px; border-bottom: 1px solid rgba(0, 255, 255, 0.1); display: flex; justify-content: space-between; align-items: center; }
    .title { color: #a78bfa; font-weight: 900; font-size: 11px; letter-spacing: 0.15em; }
    .count { color: #64748b; font-size: 10px; font-weight: bold; }
    .kanban-grid { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px; padding: 16px; flex: 1; min-height: 0; }
    .kanban-col { display: flex; flex-direction: column; gap: 10px; min-height: 0; }
    .col-hdr { font-size: 10px; font-weight: 900; letter-spacing: 0.1em; border-bottom: 1px solid rgba(255, 255, 255, 0.05); padding-bottom: 6px; }
    .col-body { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; padding-right: 4px; scrollbar-width: thin; scrollbar-color: #374151 transparent; }
    .task-card { padding: 12px; border: 1px solid; border-radius: 4px; display: flex; flex-direction: column; gap: 6px; }
    .task-id { font-size: 8px; font-weight: bold; opacity: 0.7; }
    .task-dir { 
        font-size: 11px; 
        color: #e2e8f0; 
        line-height: 1.4; 
        display: -webkit-box; 
        -webkit-line-clamp: 3; 
        line-clamp: 3; 
        -webkit-box-orient: vertical; 
        overflow: hidden; 
    }
    .loader { height: 2px; width: 100%; background: linear-gradient(90deg, transparent, #22d3ee, transparent); animation: scan 1.5s infinite linear; background-size: 200% 100%; }
    @keyframes scan { 0% { background-position: 100% 0; } 100% { background-position: -100% 0; } }
</style>
