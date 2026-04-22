// TaskService.js — v1.1
// Central service for queuing directives and logging to terminal.
// Works with the fixed store.svelte.js / nexus_socket.svelte.ts stack.
import { storeAddLog } from '$lib/core/store.svelte.js';

export const TaskService = {
    log(msg, cls = 'text-gray-400') {
        storeAddLog(msg, cls);
    },

    async runDirective(directive, source = 'WORKSPACE') {
        this.log(`>> [${source}] Queuing: ${directive}`, 'text-cyan-400');
        try {
            const res = await fetch('/api/task', {
                method:  'POST',
                headers: { 'Content-Type': 'application/json' },
                body:    JSON.stringify({ directive }),
            });
            if (res.ok) {
                this.log(`>> [${source}] Task queued — responses appear below.`, 'text-emerald-400');
            } else {
                this.log(`!! [${source}] Server error: ${res.status}`, 'text-rose-400');
            }
        } catch (e) {
            this.log(`!! [${source}] Network fault: ${e.message}`, 'text-rose-400');
        }
    }
};
