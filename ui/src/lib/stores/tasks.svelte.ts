// tasks.svelte.ts — v1.2 [CAPPED + UPLINK + CREATED_AT]
// FIX: Task array capped at MAX_TASKS — was unbounded.
// FIX: Uses uplink instead of secureFetch for consistent auth header.
// FIX: createdAt timestamp added for sorting/eviction.
// FIX: Active tasks protected from eviction when pruning.
import { uplink } from '$lib/services/uplink.js';

export type Task = {
    id:        string;
    directive: string;
    status:    'queued' | 'running' | 'done' | 'failed';
    createdAt: number;
};

const MAX_TASKS = 200;

export const taskQueue = $state({ tasks: [] as Task[] });

function prune() {
    if (taskQueue.tasks.length <= MAX_TASKS) return;
    // Evict oldest completed tasks first, never evict active ones
    const active    = taskQueue.tasks.filter(t => t.status === 'queued' || t.status === 'running');
    const completed = taskQueue.tasks.filter(t => t.status === 'done'   || t.status === 'failed');
    const keep      = completed.slice(-(MAX_TASKS - active.length));
    taskQueue.tasks = [...active, ...keep].sort((a, b) => a.createdAt - b.createdAt);
}

export async function dispatchTask(directive: string): Promise<void> {
    const task: Task = {
        id:        crypto.randomUUID(),
        directive,
        status:    'queued',
        createdAt: Date.now(),
    };
    taskQueue.tasks = [...taskQueue.tasks, task];
    prune();

    try {
        task.status = 'running';
        const res = await uplink.post('/api/task', { directive });
        task.status = res.ok ? 'done' : 'failed';
    } catch {
        task.status = 'failed';
    }
}
